# release.ps1 — Create and push a version tag to trigger a GitHub Actions release.
#
# This script tags the current commit with a version like v4.1.0 and pushes it.
# The version must match "version" in src-tauri/tauri.conf.json.
# GitHub Actions will then automatically:
#   1. Build installers for Windows, macOS and Linux (x64 and ARM64)
#   2. Publish a GitHub Release once every installer is attached
#
# Usage:
#   .\release.ps1 v2.1.0
#
# Requirements:
#   - You must be on the main branch with a clean working tree
#   - The tag must not already exist

param(
    [Parameter(Mandatory=$true, Position=0)]
    [string]$Version
)

$ErrorActionPreference = "Stop"

# Ensure the tag follows the v* convention (e.g. v2.1.0)
if ($Version -notmatch '^v\d+\.\d+\.\d+$') {
    Write-Error "Version must match the pattern vMAJOR.MINOR.PATCH (e.g. v2.1.0)"
    exit 1
}

# The tag must match the app version, or the release workflow refuses to build
$appVersion = "v" + (Get-Content src-tauri/tauri.conf.json -Raw | ConvertFrom-Json).version
if ($Version -ne $appVersion) {
    Write-Error "$Version doesn't match the app version $appVersion in src-tauri/tauri.conf.json. Bump `"version`" there, commit, then run this again."
    exit 1
}

# Check if the tag already exists locally
$existing = git tag -l $Version
if ($existing) {
    Write-Error "Tag $Version already exists"
    exit 1
}

# Create an annotated tag on the current commit
git tag -a $Version -m "Release $Version"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# Push the tag to GitHub — this triggers the release workflow
git push origin $Version
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "Tag $Version pushed. Check the Actions tab on GitHub for build progress."
