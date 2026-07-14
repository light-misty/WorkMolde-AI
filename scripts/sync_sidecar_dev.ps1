# WorkMolde AI Dev Mode Sidecar Sync Script
# Sync sidecar/ source to src-tauri/target/debug/sidecar_dist/sidecar/
#
# Purpose:
#   1. Double protection: even if Rust path resolution changes, dev mode won't load stale sidecar_dist
#   2. Clean __pycache__ and .pyc caches to avoid Python loading stale bytecode
#   3. Provide log output for developers to confirm sync status
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts/sync_sidecar_dev.ps1
#   Or auto-invoked via npm run pretauri:dev

$ErrorActionPreference = "Stop"

# Path config
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir
$SidecarSourceDir = Join-Path $ProjectRoot "sidecar"
$DebugTargetDir = Join-Path $ProjectRoot "src-tauri\target\debug\sidecar_dist\sidecar"

function Write-Info {
    param([string]$Message)
    Write-Host "[sync-sidecar] $Message" -ForegroundColor Cyan
}

# 1. Check source directory exists
if (-not (Test-Path $SidecarSourceDir)) {
    Write-Host "[sync-sidecar] Source directory not found: $SidecarSourceDir" -ForegroundColor Red
    exit 1
}

# 2. If target directory exists, clean __pycache__ and .pyc first
if (Test-Path $DebugTargetDir) {
    Write-Info "Cleaning __pycache__ and .pyc in target: $DebugTargetDir"

    # Remove all __pycache__ directories
    Get-ChildItem -Path $DebugTargetDir -Recurse -Directory -Filter "__pycache__" -ErrorAction SilentlyContinue |
        ForEach-Object {
            Remove-Item -Path $_.FullName -Recurse -Force -ErrorAction SilentlyContinue
        }

    # Remove all .pyc files (keep .py source files)
    Get-ChildItem -Path $DebugTargetDir -Recurse -Filter "*.pyc" -ErrorAction SilentlyContinue |
        ForEach-Object {
            Remove-Item -Path $_.FullName -Force -ErrorAction SilentlyContinue
        }
} else {
    Write-Info "Target directory not found, creating: $DebugTargetDir"
    New-Item -ItemType Directory -Path $DebugTargetDir -Force | Out-Null
}

# 3. Sync source to target (preserve directory structure, exclude __pycache__ and tests)
Write-Info "Syncing sidecar source to dev mode sidecar_dist..."

# Use robocopy to sync, exclude __pycache__, tests, .pyc
# /MIR mirror sync, /XD exclude directories, /XF exclude files
$robocopyArgs = @(
    $SidecarSourceDir,
    $DebugTargetDir,
    "/MIR",
    "/XD", "__pycache__", "tests",
    "/XF", "*.pyc", "*.pyo",
    "/NFL", "/NDL", "/NJH", "/NJS", "/NC", "/NS", "/NP"
)

& robocopy @robocopyArgs

# robocopy exit codes 0-7 mean success, 8+ mean failure
if ($LASTEXITCODE -ge 8) {
    Write-Host "[sync-sidecar] robocopy failed, exit code: $LASTEXITCODE" -ForegroundColor Red
    exit 1
}

Write-Info "Sync complete: $SidecarSourceDir -> $DebugTargetDir"
Write-Info "Dev mode sidecar_dist updated to latest source"
