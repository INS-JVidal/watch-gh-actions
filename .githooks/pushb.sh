#!/bin/sh
#
# pushb — push with automatic patch version bump.
#
# Usage: git pushb  (via git alias)
#    or: .githooks/pushb.sh
#
# This replaces the old pre-push hook approach, which caused spurious
# "error: failed to push some refs" messages from git.

set -e

remote="${1:-origin}"
branch=$(git symbolic-ref --short HEAD)

# Pull any remote changes (e.g. CI MINOR bumps)
git pull --rebase --no-verify "$remote" "$branch" 2>/dev/null || true

# Parse current version from Cargo.toml
current=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
major=$(echo "$current" | cut -d. -f1)
minor=$(echo "$current" | cut -d. -f2)
patch=$(echo "$current" | cut -d. -f3)

# Bump patch
new_patch=$((patch + 1))
new_version="$major.$minor.$new_patch"

sed -i "s/^version = \"$current\"/version = \"$new_version\"/" Cargo.toml
git add Cargo.toml
git commit --no-verify -m "chore: bump patch to $new_version"

# Push everything
git push --no-verify "$remote" "$branch"

echo "Pushed with patch bump: $current -> $new_version"
