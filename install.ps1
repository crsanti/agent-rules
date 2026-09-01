# Installs the agent-rules binary for Windows from a GitHub release.
# No param() block: this script is meant to run as `irm <url> | iex`,
# where there is no script file for a parameter block to bind against --
# all configuration comes from environment variables instead
# ($env:AGENT_RULES_INSTALL_DIR, $env:AGENT_RULES_VERSION).
#
# The whole body lives inside a function, called once at the bottom,
# rather than running at the top level: under `irm <url> | iex` this
# script runs in the CALLER's session scope, not its own process, so a
# top-level `exit` would close the user's entire PowerShell window (see
# PowerShell/PowerShell#8816) and a top-level `$ErrorActionPreference`
# assignment would leak into their session after the install finishes.
# Wrapping in a function scopes both the preference change and every
# `throw` to the function call, so the session survives either way.
function Install-AgentRules {
    $ErrorActionPreference = 'Stop'
    # Windows PowerShell 5.1's default progress-bar rendering makes
    # Invoke-WebRequest dramatically slower on a plain download; this
    # script doesn't need the progress UI at all.
    $ProgressPreference = 'SilentlyContinue'

    # PS 5.1's default SecurityProtocol on older Windows can exclude TLS
    # 1.2, which GitHub requires. -bor adds the flag without clobbering
    # whatever else is already enabled.
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

    $repo = 'crsanti/agent-rules'

    $asset = switch ($env:PROCESSOR_ARCHITECTURE) {
        'AMD64' { 'agent-rules-windows-amd64.exe' }
        default {
            throw "agent-rules: install: no prebuilt binary for windows-$($env:PROCESSOR_ARCHITECTURE) (supported: windows-AMD64); build from source instead: mise run build"
        }
    }

    $version = $env:AGENT_RULES_VERSION
    if ($version) {
        $version = $version.TrimStart('v')
        $url = "https://github.com/$repo/releases/download/v$version/$asset"
    } else {
        $url = "https://github.com/$repo/releases/latest/download/$asset"
    }

    $installDir = $env:AGENT_RULES_INSTALL_DIR
    if (-not $installDir) {
        # Deliberately the same location the sh installer produces under
        # Git Bash, so Windows has one canonical install path regardless
        # of which installer someone runs.
        $installDir = Join-Path $env:USERPROFILE '.local\bin'
    }
    New-Item -ItemType Directory -Force -Path $installDir | Out-Null

    $destFile = Join-Path $installDir 'agent-rules.exe'
    $tmpFile = Join-Path $installDir ".agent-rules.tmp.$PID.exe"

    try {
        Write-Host "agent-rules install: downloading $asset..."
        Invoke-WebRequest -Uri $url -OutFile $tmpFile -UseBasicParsing
        # dest_file only ever appears at its final path once fully written,
        # never partway through the download.
        Move-Item -Force -Path $tmpFile -Destination $destFile
    } finally {
        if (Test-Path $tmpFile) {
            Remove-Item -Force $tmpFile -ErrorAction SilentlyContinue
        }
    }

    $normalizedInstallDir = $installDir.TrimEnd('\')
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $alreadyOnPath = $false
    if ($userPath) {
        foreach ($entry in $userPath.Split(';')) {
            if ($entry.Trim().TrimEnd('\') -eq $normalizedInstallDir) {
                $alreadyOnPath = $true
                break
            }
        }
    }
    if (-not $alreadyOnPath) {
        # SetEnvironmentVariable(...,'User') writes the registry value
        # directly with no length limit; `setx` is never used here
        # because it truncates PATH at 1024 characters and can silently
        # corrupt it.
        $newPath = if ($userPath) { "$userPath;$installDir" } else { $installDir }
        [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
        $env:Path = "$env:Path;$installDir"
        Write-Host ""
        Write-Host "agent-rules install: added $installDir to your user PATH."
        Write-Host "agent-rules install: restart any other open shells to pick it up."
    }

    Write-Host ""
    # Not wrapped in try/catch: a non-zero exit code from a native exe
    # does not raise a terminating error under $ErrorActionPreference, so
    # this already can't abort the install -- $LASTEXITCODE just flags
    # whether an AGENT_RULES_VERSION pinned to a release old enough to
    # predate the `version` subcommand printed its own usage instead of a
    # version string.
    & $destFile version
    if ($LASTEXITCODE -ne 0) {
        Write-Host "agent-rules install: installed at $destFile (couldn't run 'agent-rules version' to confirm -- this release may predate that subcommand)"
    }
    Write-Host ""
    Write-Host "Quickstart: agent-rules apply"
}

Install-AgentRules
