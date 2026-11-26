#!/usr/bin/env bash
# Publishes dist/* as a GitHub release of the checked-out commit, creating its tag.
#
# Usage: ci/release.sh <tag>
# Needs GH_TOKEN (allowed to write the repository's contents) and GITHUB_REPO ("owner/name").
# With RVLC_UPDATE_SIGNING_KEY (or RVLC_UPDATE_SIGNING_KEY_FILE) set, it also writes and signs
# the update manifest the app checks (S9.5, docs/adr/0005-updates.md); without it the release has
# none, and installed apps find no update in it.
set -euo pipefail

tag="$1"
repo="${GITHUB_REPO:?GITHUB_REPO is not set}"
commit="$(git rev-parse HEAD)"

if gh release view "$tag" --repo "$repo" > /dev/null 2>&1; then
    echo "Release $tag already exists: bump the version in Cargo.toml" >&2
    exit 1
fi

if [[ -n "${RVLC_UPDATE_SIGNING_KEY:-}${RVLC_UPDATE_SIGNING_KEY_FILE:-}" ]]; then
    cargo run --quiet --example update_manifest -- sign dist
else
    echo "No update signing key: the release has no update manifest" >&2
fi

flags=(--repo "$repo" --target "$commit" --title "RedesignedVLC $tag" --generate-notes --latest)

gh release create "$tag" dist/* "${flags[@]}"
echo "Published https://github.com/$repo/releases/tag/$tag"
