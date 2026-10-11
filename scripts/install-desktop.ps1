param(
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA "Rumux"),
    [switch]$NoPath,
    [switch]$NoShortcut
)
$ErrorActionPreference = "Stop"
# Verify the extracted files before changing the per-user installation.
foreach ($line in Get-Content (Join-Path $PSScriptRoot "files.sha256")) {
    if ($line -notmatch '^([0-9a-f]{64})  (.+)$') { throw "Invalid package checksum entry" }
    $expected = $Matches[1]
    $file = Join-Path $PSScriptRoot $Matches[2]
    if ((Get-FileHash -Algorithm SHA256 -LiteralPath $file).Hash.ToLowerInvariant() -ne $expected) {
        throw "Package checksum failed: $file"
    }
}
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
$InstallDir = (Resolve-Path -LiteralPath $InstallDir).Path
foreach ($name in @("bin", "licenses", "package.json", "README.txt")) {
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) -Destination $InstallDir -Recurse -Force
}
$bin = Join-Path $InstallDir "bin"
if (-not $NoPath) {
    $current = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($bin -notin ($current -split ';')) {
        [Environment]::SetEnvironmentVariable("Path", (([string]$current).TrimEnd(';') + ';' + $bin).TrimStart(';'), "User")
    }
}
if (-not $NoShortcut) {
    $menu = [Environment]::GetFolderPath("Programs")
    $shell = New-Object -ComObject WScript.Shell
    $shortcut = $shell.CreateShortcut((Join-Path $menu "Rumux.lnk"))
    $shortcut.TargetPath = Join-Path $bin "rumux-app.exe"
    $shortcut.WorkingDirectory = $env:USERPROFILE
    $shortcut.Description = "Rumux terminal workspaces"
    $shortcut.Save()
}
Write-Output "Installed Rumux desktop and CLI in $InstallDir. Open a new shell for PATH changes."
