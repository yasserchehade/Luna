$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '_native-command.ps1')

try {
  $success = Invoke-LunaNativeCommand -FilePath 'node' -ArgumentList @(
    '-e',
    "console.error('Container luna-litellm-gateway-1 Stopping'); process.exit(0)"
  )
}
catch {
  Write-Output "Native command boundary: fail ($($_.FullyQualifiedErrorId))"
  exit 1
}

if ($success.ExitCode -ne 0) {
  throw "Expected the successful native command to return exit code 0; got $($success.ExitCode)."
}

if (($success.Output | Out-String) -notmatch 'Container luna-litellm-gateway-1 Stopping') {
  throw 'Expected stderr progress output to remain available to the caller.'
}

$failure = Invoke-LunaNativeCommand -FilePath 'node' -ArgumentList @(
  '-e',
  "console.error('controlled failure'); process.exit(7)"
)

if ($failure.ExitCode -ne 7) {
  throw "Expected the failing native command to return exit code 7; got $($failure.ExitCode)."
}

Write-Output 'Native command boundary: pass'
exit 0
