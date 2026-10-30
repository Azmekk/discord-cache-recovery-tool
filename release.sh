#!/bin/bash

# release.sh — Create and push a version tag to trigger a GitHub Actions release.
#
# This script tags the current commit with a version like v4.1.0 and pushes it.
# The version must match "version" in src-tauri/tauri.conf.json.
# GitHub Actions will then automatically:
#   1. Build installers for Windows, macOS and Linux (x64 and ARM64)
#   2. Publish a GitHub Release once every installer is attached
#
# Usage:
#   ./release.sh v2.1.0
#
# Requirements:
#   - You must be on the main branch with a clean working tree
#   - The tag must not already exist

set -e

if [ -z "$1" ]; then
  echo "Usage: ./release.sh <version>"
  echo "Example: ./release.sh v2.1.0"
  exit 1
fi

VERSION="$1"

# Ensure the tag follows the v* convention (e.g. v2.1.0)
if [[ ! "$VERSION" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Error: Version must match the pattern vMAJOR.MINOR.PATCH (e.g. v2.1.0)"
  exit 1
fi

# The tag must match the app version, or the release workflow refuses to build
APP_VERSION="v$(node -p "require('./src-tauri/tauri.conf.json').version")"
if [ "$VERSION" != "$APP_VERSION" ]; then
  echo "Error: $VERSION doesn't match the app version $APP_VERSION in src-tauri/tauri.conf.json."
  echo "Bump \"version\" there, commit, then run this again."
  exit 1
fi

# Check if the tag already exists locally or on the remote
if git rev-parse "$VERSION" >/dev/null 2>&1; then
  echo "Error: Tag $VERSION already exists"
  exit 1
fi

# Create an annotated tag on the current commit
git tag -a "$VERSION" -m "Release $VERSION"

# Push the tag to GitHub — this triggers the release workflow
git push origin "$VERSION"

echo "Tag $VERSION pushed. Check the Actions tab on GitHub for build progress."
