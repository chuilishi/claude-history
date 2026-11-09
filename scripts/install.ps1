# claude-history installation script for Windows
# Usage: irm https://raw.githubusercontent.com/chuilishi/claude-history/main/scripts/install.ps1 | iex
#
# Environment variables:
#   CLAUDE_HISTORY_VERSION      - Pin a specific version (e.g., v0.1.30)
#   CLAUDE_HISTORY_INSTALL_DIR  - Override install directory
#                                 (default: %LOCALAPPDATA%\Programs\claude-history)
#   CLAUDE_HISTORY_NO_MODIFY_PATH - Set to any value to leave the user PATH alone
#
# Examples:
#   $env:CLAUDE_HISTORY_VERSION = "v0.1.30"; irm .../install.ps1 | iex
#
# Works with Windows PowerShell 5.1 and PowerShell 7+. Running the same
# command again upgrades an existing installation.

# Everything runs inside a function: `iex` executes in the caller's session,
# so `exit` would close the user's terminal and variables would leak into it.
function Install-ClaudeHistory {
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue'  # the progress bar makes downloads very slow on 5.1

    $repo = 'chuilishi/claude-history'
    $binName = 'claude-history'
    $platform = 'windows-amd64'

    function Write-Info($msg) { Write-Host "==> $msg" -ForegroundColor Blue }
    function Write-Ok($msg) { Write-Host "==> $msg" -ForegroundColor Green }
    function Write-Warn($msg) { Write-Host "Warning: $msg" -ForegroundColor Yellow }

    # Download with retries; proxies and GitHub's CDN fail transiently (504s)
    function Get-File($url, $outFile) {
        $attempts = 3
        for ($i = 1; $i -le $attempts; $i++) {
            try {
                Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $outFile
                return
            } catch {
                if ($i -eq $attempts) { throw }
                Write-Warn "Download failed ($($_.Exception.Message.Trim())), retrying ($i/$attempts)..."
                Start-Sleep -Seconds (2 * $i)
            }
        }
    }

    # `vX.Y.Z` from `<exe> --version`, or $null. 5.1 turns native stderr
    # output into errors under 'Stop', hence the try.
    function Get-BinaryVersion($path) {
        try {
            $out = & $path --version 2>$null
            if ("$out" -match 'v\d+\.\d+\.\d+') { return $Matches[0] }
        } catch { }
        return $null
    }

    # Windows PowerShell 5.1 may default to TLS 1.0, which GitHub rejects
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

    $arch = $env:PROCESSOR_ARCHITECTURE
    if ($env:PROCESSOR_ARCHITEW6432) { $arch = $env:PROCESSOR_ARCHITEW6432 }
    switch ($arch) {
        'AMD64' { }
        'ARM64' { Write-Warn 'No native ARM64 build; installing the x64 binary, which runs under emulation' }
        default {
            throw "Unsupported architecture: $arch. Try building from source: cargo install claude-history-resume"
        }
    }

    # `releases/latest/download` redirects to the newest release's file. Unlike
    # the GitHub API it is not rate limited (60 requests/hour per IP, often
    # exhausted behind shared proxies); the version is read from the binary.
    $version = $env:CLAUDE_HISTORY_VERSION
    if ($version) {
        if (-not $version.StartsWith('v')) { $version = "v$version" }
        Write-Info "Installing pinned version: $version"
        $base = "https://github.com/$repo/releases/download/$version"
    } else {
        Write-Info 'Installing the latest release'
        $base = "https://github.com/$repo/releases/latest/download"
    }

    $installDir = $env:CLAUDE_HISTORY_INSTALL_DIR
    if (-not $installDir) { $installDir = Join-Path $env:LOCALAPPDATA "Programs\$binName" }
    New-Item -ItemType Directory -Force -Path $installDir | Out-Null
    $target = Join-Path $installDir "$binName.exe"

    $assetName = "$binName-$platform.exe"
    $tmpDir = Join-Path ([IO.Path]::GetTempPath()) ("$binName-install-" + [Guid]::NewGuid())
    New-Item -ItemType Directory -Path $tmpDir | Out-Null

    try {
        $exe = Join-Path $tmpDir $assetName
        $sumFile = Join-Path $tmpDir "$binName-$platform.sha256"

        Write-Info "Downloading $assetName..."
        try {
            Get-File "$base/$assetName" $exe
        } catch {
            throw "Failed to download $base/$assetName. The release may not have a Windows binary. ($_)"
        }

        Write-Info 'Verifying checksum...'
        try {
            Get-File "$base/$binName-$platform.sha256" $sumFile
        } catch {
            throw "Failed to download the checksum file. ($_)"
        }
        # Format: "<hash>  <file name>"
        $expected = ((Get-Content -Raw $sumFile).Trim() -split '\s+')[0].ToLower()
        # .NET directly: Get-FileHash lives in a module that may fail to load
        $sha = [Security.Cryptography.SHA256]::Create()
        $stream = [IO.File]::OpenRead($exe)
        try {
            $actual = -join ($sha.ComputeHash($stream) | ForEach-Object { $_.ToString('x2') })
        } finally {
            $stream.Dispose()
            $sha.Dispose()
        }
        if ($expected -ne $actual) {
            throw "Checksum mismatch: expected $expected, got $actual"
        }
        Write-Ok 'Checksum verified'

        $newVersion = Get-BinaryVersion $exe
        if (-not $newVersion) { $newVersion = $version }
        if (Test-Path $target) {
            $existing = Get-BinaryVersion $target
            if (-not $existing) { $existing = 'unknown' }
            Write-Info "Existing installation found: $existing"
            Write-Info "Upgrading to: $newVersion"
        } elseif ($newVersion) {
            Write-Info "Version: $newVersion"
        }

        try {
            Move-Item -Force -Path $exe -Destination $target
        } catch {
            throw "Could not replace $target. Close any running claude-history and try again. ($_)"
        }
        Write-Ok "Installed to $target"
    } finally {
        Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $tmpDir
    }

    # Add to the user PATH (persisted) and to this session
    if ($env:CLAUDE_HISTORY_NO_MODIFY_PATH) {
        Write-Info "Leaving PATH unchanged; add $installDir to it yourself"
    } else {
        $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
        $entries = @()
        if ($userPath) { $entries = @($userPath -split ';' | Where-Object { $_ }) }
        if ($entries -notcontains $installDir) {
            [Environment]::SetEnvironmentVariable('Path', (($entries + $installDir) -join ';'), 'User')
            Write-Ok "Added $installDir to your user PATH (new terminals pick it up)"
        }
    }
    if (($env:Path -split ';') -notcontains $installDir) {
        $env:Path = "$env:Path;$installDir"
    }

    # 5.1 turns native stderr output into errors under 'Stop'; check the exit code instead
    $installed = $null
    try { $installed = & $target --version 2>$null } catch { }
    if ($LASTEXITCODE -ne 0 -or -not $installed) {
        throw "claude-history was installed to $target but failed to run"
    }
    Write-Ok 'claude-history is installed and ready!'
    Write-Host ''
    Write-Host "  $installed"
    Write-Host ''
    Write-Host 'Run: claude-history'
}

Install-ClaudeHistory
