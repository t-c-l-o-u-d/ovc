#!/bin/bash

# Script to bump version in Cargo.toml and trigger a release
# Usage: ./scripts/bump-version.sh [major|minor|patch] or ./scripts/bump-version.sh <specific-version>

set -e

if [ $# -ne 1 ]; then
    echo "Usage: $0 [major|minor|patch|<specific-version>]"
    echo "Examples:"
    echo "  $0 patch      # 0.1.0 -> 0.1.1"
    echo "  $0 minor      # 0.1.0 -> 0.2.0"
    echo "  $0 major      # 0.1.0 -> 1.0.0"
    echo "  $0 1.2.3      # Set specific version"
    exit 1
fi

# Get current version from Cargo.toml
current_version=$(grep '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/')
echo "Current version: $current_version"

# Parse version components
IFS='.' read -r major minor patch <<< "$current_version"

case "$1" in
    "major")
        new_version="$((major + 1)).0.0"
        ;;
    "minor")
        new_version="$major.$((minor + 1)).0"
        ;;
    "patch")
        new_version="$major.$minor.$((patch + 1))"
        ;;
    *)
        # Assume it's a specific version
        if [[ "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
            new_version="$1"
        else
            echo "Error: Invalid version format. Use major.minor.patch (e.g., 1.2.3)"
            exit 1
        fi
        ;;
esac

echo "New version: $new_version"

# Update Cargo.toml
sed -i.bak "s/^version = \".*\"/version = \"$new_version\"/" Cargo.toml

# Remove backup file
rm Cargo.toml.bak

echo "Updated Cargo.toml with version $new_version"

# Check if we're in a git repository
if git rev-parse --git-dir > /dev/null 2>&1; then
    echo ""
    echo "Next steps:"
    echo "1. Review the changes: git diff"
    echo "2. Commit the version bump: git add Cargo.toml && git commit -m \"Bump version to $new_version\""
    echo "3. Push to main: git push origin main"
    echo "4. The release workflow will automatically create a release when the change is pushed to main"
else
    echo "Not in a git repository. Please manually commit and push the changes."
fi 