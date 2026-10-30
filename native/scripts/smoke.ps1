# Starts the built Moa, checks that it is still running after a few seconds
# and prints its memory use. Used by CI and the release workflow.
param([Parameter(Mandatory)][string]$Exe)

$env:MOA_CONFIG = Join-Path $env:RUNNER_TEMP 'moa-smoke-config.json'
$p = Start-Process -FilePath $Exe -PassThru
Start-Sleep -Seconds 12
$p.Refresh()
if ($p.HasExited) { throw "Moa exited after start (code $($p.ExitCode))" }
'Moa is running. Private memory: {0:N1} MB, working set: {1:N1} MB' -f ($p.PrivateMemorySize64 / 1MB), ($p.WorkingSet64 / 1MB)
Stop-Process -Id $p.Id -Force
