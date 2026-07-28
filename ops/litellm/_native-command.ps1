function Invoke-LunaNativeCommand {
  [CmdletBinding()]
  param(
    [Parameter(Mandatory = $true)]
    [string]$FilePath,

    [string[]]$ArgumentList = @()
  )

  $previousErrorActionPreference = $ErrorActionPreference
  try {
    # Windows PowerShell 5.1 turns native stderr into ErrorRecord objects. Under
    # Stop, ordinary Docker progress can terminate the script before the real
    # process exit code is available. Capture both streams under Continue and
    # let callers decide success from ExitCode.
    $ErrorActionPreference = 'Continue'
    $output = @(& $FilePath @ArgumentList 2>&1)
    $exitCode = $LASTEXITCODE
  }
  finally {
    $ErrorActionPreference = $previousErrorActionPreference
  }

  [pscustomobject]@{
    ExitCode = $exitCode
    Output = $output
  }
}
