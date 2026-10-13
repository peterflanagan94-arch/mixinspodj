#!/usr/bin/env bash
# Builds and publishes a new MixInspo DJ release in one step:
# bumps the version everywhere it's recorded, runs the test suite,
# builds the .dmg, commits, tags, and publishes a GitHub release with
# the .dmg attached.
#
# Usage: scripts/release.sh 0.3.0

set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="${1:-}"
if [[ -z "$VERSION" ]]; then
  echo "Usage: scripts/release.sh <version, e.g. 0.3.0>" >&2
  exit 1
fi
VERSION="${VERSION#v}"
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Version must look like 0.3.0 (got: $VERSION)" >&2
  exit 1
fi
TAG="v$VERSION"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "Working tree isn't clean — commit or stash first." >&2
  exit 1
fi

BRANCH="$(git rev-parse --abbrev-ref HEAD)"
if [[ "$BRANCH" != "main" ]]; then
  echo "Warning: releasing from '$BRANCH', not 'main'." >&2
fi

if git rev-parse "$TAG" >/dev/null 2>&1; then
  echo "Tag $TAG already exists — pick a version that hasn't been used." >&2
  exit 1
fi

echo "==> Bumping version to $VERSION"
sed -i '' -E "0,/\"version\": \"[0-9]+\.[0-9]+\.[0-9]+\"/s//\"version\": \"$VERSION\"/" src-tauri/tauri.conf.json
sed -i '' -E "0,/\"version\": \"[0-9]+\.[0-9]+\.[0-9]+\"/s//\"version\": \"$VERSION\"/" package.json
sed -i '' -E "0,/^version = \"[0-9]+\.[0-9]+\.[0-9]+\"/s//version = \"$VERSION\"/" src-tauri/Cargo.toml

echo "==> Running the test suite"
cargo test --workspace --quiet

echo "==> Refreshing Cargo.lock for the version bump"
cargo check --workspace --quiet

echo "==> Committing the version bump"
git add src-tauri/tauri.conf.json package.json src-tauri/Cargo.toml Cargo.lock
git commit -m "Bump version to $VERSION"

echo "==> Building the .dmg"
npm run tauri build -- --bundles dmg

DMG_PATH=$(find target/release/bundle/dmg -maxdepth 1 -name "*.dmg" | head -1)
if [[ -z "$DMG_PATH" ]]; then
  echo "Couldn't find the built .dmg under target/release/bundle/dmg" >&2
  exit 1
fi

echo "==> Pushing to origin"
git push origin "$BRANCH"

echo "==> Creating GitHub release $TAG"
gh release create "$TAG" "$DMG_PATH" \
  --title "MixInspo DJ $TAG" \
  --target "$BRANCH" \
  --notes "Not signed or notarized yet, so macOS will warn it can't check it for malware on first open.

**To open it:** double-click the .dmg, drag MixInspo DJ to Applications, then right-click (or Control-click) the app in Applications and choose **Open** — confirm **Open** again in the dialog. You only need to do this once.

Apple Silicon (M-series) Macs can run this via Rosetta 2, which macOS will offer to install automatically the first time you open it."

ASSET_URL=$(gh release view "$TAG" --json assets --jq '.assets[0].url')
echo ""
echo "==> Done. Download link:"
echo "$ASSET_URL"
echo ""
echo "Update DOWNLOAD_URL in the landing page (studio-on-screen's src/app/mixinspo-dj/page.tsx) to this new link."
