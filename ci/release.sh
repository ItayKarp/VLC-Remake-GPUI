#!/usr/bin/env bash
# Publishes dist/* as a GitHub release of the checked-out commit, creating its tag.
#
# Usage: ci/release.sh <tag>
# Needs GH_TOKEN (allowed to write the repository's contents) and GITHUB_REPO ("owner/name").
set -euo pipefail

tag="$1"
repo="${GITHUB_REPO:?GITHUB_REPO is not set}"
commit="$(git rev-parse HEAD)"

if gh release view "$tag" --repo "$repo" > /dev/null 2>&1; then
    echo "Release $tag already exists: bump the version in Cargo.toml" >&2
    exit 1
fi

flags=(--repo "$repo" --target "$commit" --title "RedesignedVLC $tag" --generate-notes --latest)

gh release create "$tag" dist/* "${flags[@]}"
echo "Published https://github.com/$repo/releases/tag/$tag"
