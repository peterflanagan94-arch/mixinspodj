#!/usr/bin/env bash
# Builds and publishes a new MixInspo DJ release in one step:
# bumps the version everywhere it's recorded, runs the test suite,
# builds a universal (Intel + Apple Silicon) .dmg, commits, tags, and
# publishes a GitHub release with the .dmg attached.
#
# To sign and notarize, export these before running (Tauri picks them up):
#   APPLE_SIGNING_IDENTITY  e.g. "Developer ID Application: Name (TEAMID)"
#   APPLE_ID, APPLE_PASSWORD (app-specific password), APPLE_TEAM_ID
# Without them the build is unsigned and the release notes say so.
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

echo "==> Building the universal .dmg"
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run tauri build -- --target universal-apple-darwin --bundles dmg

DMG_PATH=$(find target/universal-apple-darwin/release/bundle/dmg -maxdepth 1 -name "*.dmg" | head -1)
if [[ -z "$DMG_PATH" ]]; then
  echo "Couldn't find the built .dmg under target/universal-apple-darwin/release/bundle/dmg" >&2
  exit 1
fi

echo "==> Pushing to origin"
git push origin "$BRANCH"

if [[ -n "${APPLE_SIGNING_IDENTITY:-}" ]]; then
  NOTES="Signed and notarized. Double-click the .dmg and drag MixInspo DJ to Applications. Works on Intel and Apple Silicon Macs."
else
  NOTES="Not signed or notarized yet, so macOS will warn it can't check it for malware on first open.

**To open it:** double-click the .dmg and drag MixInspo DJ to Applications. Try to open the app once, then go to **System Settings → Privacy & Security**, scroll down and click **Open Anyway**, and confirm. You only need to do this once.

Works on Intel and Apple Silicon Macs."
fi

echo "==> Creating GitHub release $TAG"
gh release create "$TAG" "$DMG_PATH" \
  --title "MixInspo DJ $TAG" \
  --target "$BRANCH" \
  --notes "$NOTES"

echo ""
echo "==> Done. The landing page links to the latest release, so it needs no edit."
