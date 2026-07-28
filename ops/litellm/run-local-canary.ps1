[CmdletBinding()]
param(
  [string]$EncryptedKeyPath = (Join-Path $env:LOCALAPPDATA 'Luna\openai-canary-key.clixml'),
  [string]$EvidencePath = (Join-Path $env:LOCALAPPDATA 'Luna\litellm-canary-result.json')
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '_native-command.ps1')
$composeProject = 'luna-litellm'
$composeFile = Join-Path $PSScriptRoot 'compose.yaml'
$repositoryRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..')).Path
$encryptedKey = [IO.Path]::GetFullPath($EncryptedKeyPath)
$evidenceFile = [IO.Path]::GetFullPath($EvidencePath)
$expectedSecretDirectory = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'Luna'))

if ([IO.Path]::GetDirectoryName($encryptedKey) -ne $expectedSecretDirectory) {
  throw 'The encrypted key must be in the user-local Luna directory.'
}

if ([IO.Path]::GetDirectoryName($evidenceFile) -ne $expectedSecretDirectory) {
  throw 'The evidence file must be in the user-local Luna directory.'
}

if (-not (Test-Path -LiteralPath $encryptedKey -PathType Leaf)) {
  throw "Encrypted key handoff not found: $encryptedKey"
}

function New-LunaHexToken {
  param([Parameter(Mandatory = $true)][int]$ByteCount)

  $bytes = New-Object byte[] $ByteCount
  $generator = [Security.Cryptography.RandomNumberGenerator]::Create()
  try {
    $generator.GetBytes($bytes)
  }
  finally {
    $generator.Dispose()
  }

  return -join ($bytes | ForEach-Object { $_.ToString('x2') })
}

$openAiKey = $null
$masterKey = $null
$databasePassword = $null
$secureKey = $null
$secretPointer = [IntPtr]::Zero
$composeStarted = $false
$runFailure = $null
$cleanupFailure = $null
$canaryResult = $null
$logEvidence = $null
$originalLocation = (Get-Location).Path

function Remove-LunaSecretsFromText {
  param([string]$Text)

  $redacted = [string]$Text
  foreach ($secret in @($openAiKey, $masterKey, $databasePassword)) {
    if (-not [string]::IsNullOrEmpty($secret)) {
      $redacted = $redacted.Replace($secret, '[REDACTED]')
    }
  }

  return $redacted
}

function Get-LunaNativeOutputText {
  param([Parameter(Mandatory = $true)]$Result)

  return (($Result.Output | ForEach-Object { $_.ToString() }) -join [Environment]::NewLine).Trim()
}

function Get-LunaDockerResourceIds {
  param(
    [Parameter(Mandatory = $true)][ValidateSet('container', 'network', 'volume')][string]$Resource
  )

  switch ($Resource) {
    'container' {
      $result = Invoke-LunaNativeCommand -FilePath 'docker' -ArgumentList @(
        'ps', '-a', '--filter', "label=com.docker.compose.project=$composeProject", '-q'
      )
    }
    'network' {
      $result = Invoke-LunaNativeCommand -FilePath 'docker' -ArgumentList @(
        'network', 'ls', '--filter', "label=com.docker.compose.project=$composeProject", '-q'
      )
    }
    'volume' {
      $result = Invoke-LunaNativeCommand -FilePath 'docker' -ArgumentList @(
        'volume', 'ls', '--filter', "label=com.docker.compose.project=$composeProject", '-q'
      )
    }
  }

  if ($result.ExitCode -ne 0) {
    throw "Unable to inspect existing Docker $Resource resources."
  }

  return @($result.Output | ForEach-Object { $_.ToString().Trim() } | Where-Object { $_ })
}

try {
  $existingContainers = @(Get-LunaDockerResourceIds -Resource container)
  $existingNetworks = @(Get-LunaDockerResourceIds -Resource network)
  $existingVolumes = @(Get-LunaDockerResourceIds -Resource volume)
  if ($existingContainers.Count -gt 0 -or $existingNetworks.Count -gt 0 -or $existingVolumes.Count -gt 0) {
    throw 'Existing luna-litellm Docker resources were found. Inspect and remove them before running the canary.'
  }

  $secureKey = Import-Clixml -LiteralPath $encryptedKey
  if ($secureKey -isnot [Security.SecureString] -or $secureKey.Length -lt 20) {
    throw 'The encrypted OpenAI key handoff is invalid.'
  }

  $secretPointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secureKey)
  $openAiKey = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($secretPointer)
  if ([string]::IsNullOrWhiteSpace($openAiKey)) {
    throw 'The encrypted OpenAI key handoff is empty.'
  }

  $masterKey = 'sk-' + (New-LunaHexToken -ByteCount 32)
  $databasePassword = New-LunaHexToken -ByteCount 24
  $env:OPENAI_API_KEY = $openAiKey
  $env:LITELLM_MASTER_KEY = $masterKey
  $env:LITELLM_DATABASE_PASSWORD = $databasePassword
  $env:DATABASE_URL = "postgresql://llmproxy:${databasePassword}@database:5432/litellm"
  $env:LUNA_MANAGED_INTELLIGENCE_URL = 'http://127.0.0.1:4000/v1/chat/completions'

  Set-Location -LiteralPath $repositoryRoot
  $composeValidation = Invoke-LunaNativeCommand -FilePath 'docker' -ArgumentList @(
    'compose', '-f', $composeFile, 'config', '--quiet'
  )
  if ($composeValidation.ExitCode -ne 0) { throw 'Compose validation failed.' }

  $composeStartup = Invoke-LunaNativeCommand -FilePath 'docker' -ArgumentList @(
    'compose', '-f', $composeFile, 'up', '-d'
  )
  if ($composeStartup.ExitCode -ne 0) { throw 'Compose startup failed.' }
  $composeStarted = $true

  $gatewayQuery = Invoke-LunaNativeCommand -FilePath 'docker' -ArgumentList @(
    'compose', '-f', $composeFile, 'ps', '-q', 'gateway'
  )
  if ($gatewayQuery.ExitCode -ne 0) { throw 'Unable to inspect the gateway container.' }
  $gatewayId = Get-LunaNativeOutputText -Result $gatewayQuery
  if ([string]::IsNullOrWhiteSpace($gatewayId)) {
    throw 'The gateway container was not created.'
  }

  $deadline = [DateTime]::UtcNow.AddMinutes(3)
  do {
    $healthQuery = Invoke-LunaNativeCommand -FilePath 'docker' -ArgumentList @(
      'inspect', '--format', '{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}', $gatewayId
    )
    if ($healthQuery.ExitCode -ne 0) { throw 'Unable to inspect gateway health.' }
    $health = Get-LunaNativeOutputText -Result $healthQuery
    if ($health -eq 'healthy') { break }
    if ($health -eq 'unhealthy' -or [DateTime]::UtcNow -ge $deadline) {
      throw "Gateway health check ended in state: $health"
    }
    Start-Sleep -Seconds 2
  } while ($true)

  $canary = Invoke-LunaNativeCommand -FilePath 'node' -ArgumentList @(
    (Join-Path $PSScriptRoot 'canary.mjs')
  )
  $canaryOutput = Get-LunaNativeOutputText -Result $canary
  $canaryExitCode = $canary.ExitCode
  $logQuery = Invoke-LunaNativeCommand -FilePath 'docker' -ArgumentList @(
    'compose', '-f', $composeFile, 'logs', '--no-color', 'gateway', 'database'
  )
  if ($logQuery.ExitCode -ne 0) { throw 'Unable to inspect gateway logs.' }
  $gatewayLogs = Get-LunaNativeOutputText -Result $logQuery
  $logEvidence = [ordered]@{
    synthetic_marker_absent = -not $gatewayLogs.Contains('LUNA_SYNTHETIC_CANARY_53')
    upstream_key_absent = -not $gatewayLogs.Contains($openAiKey)
    master_key_absent = -not $gatewayLogs.Contains($masterKey)
    database_password_absent = -not $gatewayLogs.Contains($databasePassword)
  }

  if ($canaryExitCode -ne 0) {
    throw ('Canary failed: ' + (Remove-LunaSecretsFromText -Text $canaryOutput))
  }

  if ($logEvidence.Values -contains $false) {
    throw 'Sensitive canary material was present in the container logs.'
  }

  $canaryResult = $canaryOutput | ConvertFrom-Json
}
catch {
  $runFailure = Remove-LunaSecretsFromText -Text $_.Exception.Message
}
finally {
  if ($composeStarted) {
    $composeCleanup = Invoke-LunaNativeCommand -FilePath 'docker' -ArgumentList @(
      'compose', '-f', $composeFile, 'down', '-v', '--remove-orphans'
    )
    if ($composeCleanup.ExitCode -ne 0) {
      $cleanupFailure = 'Compose cleanup failed.'
    }
  }

  if (Test-Path -LiteralPath $encryptedKey) {
    Remove-Item -LiteralPath $encryptedKey -Force
  }

  if ($secretPointer -ne [IntPtr]::Zero) {
    [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($secretPointer)
  }

  foreach ($environmentName in @('OPENAI_API_KEY', 'LITELLM_MASTER_KEY', 'LITELLM_DATABASE_PASSWORD', 'DATABASE_URL', 'LUNA_MANAGED_INTELLIGENCE_URL')) {
    Remove-Item -LiteralPath "Env:$environmentName" -ErrorAction SilentlyContinue
  }
  $openAiKey = $null
  $masterKey = $null
  $databasePassword = $null
  $secureKey = $null
  Set-Location -LiteralPath $originalLocation
}

if ($cleanupFailure) {
  throw $cleanupFailure
}

if ($runFailure) {
  throw $runFailure
}

$remainingContainers = @(Get-LunaDockerResourceIds -Resource container)
$remainingNetworks = @(Get-LunaDockerResourceIds -Resource network)
$remainingVolumes = @(Get-LunaDockerResourceIds -Resource volume)
if ($remainingContainers.Count -gt 0 -or $remainingNetworks.Count -gt 0 -or $remainingVolumes.Count -gt 0) {
  throw 'Docker cleanup verification failed.'
}

$evidence = [ordered]@{
  canary = $canaryResult
  logs = $logEvidence
  cleanup = [ordered]@{
    encrypted_handoff_removed = -not (Test-Path -LiteralPath $encryptedKey)
    containers_removed = $true
    networks_removed = $true
    volumes_removed = $true
  }
}

$evidenceDirectory = [IO.Path]::GetDirectoryName($evidenceFile)
if (-not (Test-Path -LiteralPath $evidenceDirectory)) {
  New-Item -ItemType Directory -Path $evidenceDirectory | Out-Null
}

$evidence | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $evidenceFile -Encoding UTF8
$evidence | ConvertTo-Json -Depth 8
