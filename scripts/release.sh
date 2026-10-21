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
# (BSD sed on macOS can't do "first match only", so this uses python.)
python3 - "$VERSION" <<'PY'
import re, sys
version = sys.argv[1]
for path, pattern, template in [
    ("src-tauri/tauri.conf.json", r'"version": "\d+\.\d+\.\d+"', f'"version": "{version}"'),
    ("package.json", r'"version": "\d+\.\d+\.\d+"', f'"version": "{version}"'),
    ("src-tauri/Cargo.toml", r'(?m)^version = "\d+\.\d+\.\d+"', f'version = "{version}"'),
]:
    text = open(path).read()
    new, count = re.subn(pattern, template, text, count=1)
    if count != 1:
        sys.exit(f"Couldn't find the version in {path}")
    open(path, "w").write(new)
PY
grep -q "\"version\": \"$VERSION\"" src-tauri/tauri.conf.json || { echo "Version bump didn't apply" >&2; exit 1; }

echo "==> Running the test suite"
cargo test --workspace --quiet

echo "==> Refreshing Cargo.lock for the version bump"
cargo check --workspace --quiet

echo "==> Committing the version bump"
git add src-tauri/tauri.conf.json package.json src-tauri/Cargo.toml Cargo.lock
git commit -m "Bump version to $VERSION"

# Updates are signed with this key; keep it safe and never commit it.
KEY_FILE="${TAURI_SIGNING_PRIVATE_KEY_PATH:-$HOME/.tauri/mixinspodj.key}"
if [[ ! -f "$KEY_FILE" ]]; then
  echo "Updater signing key not found at $KEY_FILE" >&2
  exit 1
fi
export TAURI_SIGNING_PRIVATE_KEY="$KEY_FILE"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"

echo "==> Building the universal .dmg and update bundle"
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run tauri build -- --target universal-apple-darwin --bundles app,dmg

BUNDLE_DIR=target/universal-apple-darwin/release/bundle
UPDATE_TAR=$(find "$BUNDLE_DIR/macos" -maxdepth 1 -name "*.app.tar.gz" | head -1)
UPDATE_SIG="$UPDATE_TAR.sig"
if [[ -z "$UPDATE_TAR" || ! -f "$UPDATE_SIG" ]]; then
  echo "Couldn't find the signed update bundle under $BUNDLE_DIR/macos" >&2
  exit 1
fi

DMG_PATH=$(find target/universal-apple-darwin/release/bundle/dmg -maxdepth 1 -name "*.dmg" | head -1)
if [[ -z "$DMG_PATH" ]]; then
  echo "Couldn't find the built .dmg under target/universal-apple-darwin/release/bundle/dmg" >&2
  exit 1
fi

echo "==> Pushing to origin"
git push origin "$BRANCH"

if [[ -n "${APPLE_SIGNING_IDENTITY:-}" ]]; then
  NOTES="Signed and notarized. Double-click the .dmg and drag MixInspo DJ to Applications. Works on Intel and Apple Silicon Macs. The Windows installer is added a few minutes later."
else
  NOTES="Not signed or notarized yet, so macOS will warn it can't check it for malware on first open.

**To open it:** double-click the .dmg and drag MixInspo DJ to Applications. Try to open the app once, then go to **System Settings → Privacy & Security**, scroll down and click **Open Anyway**, and confirm. You only need to do this once.

Works on Intel and Apple Silicon Macs. The Windows installer is added a few minutes later."
fi

echo "==> Creating GitHub release $TAG"
# latest.json is what installed copies check; the tar.gz is a universal build, so both Mac architectures use it.
TAR_NAME=$(basename "$UPDATE_TAR" | tr ' ' '.')
LATEST_JSON="$(mktemp -d)/latest.json"
UPDATE_URL="https://github.com/peterflanagan94-arch/mixinspodj/releases/download/$TAG/$TAR_NAME"
VERSION="$VERSION" UPDATE_URL="$UPDATE_URL" SIG="$(cat "$UPDATE_SIG")" python3 - "$LATEST_JSON" <<'PY'
import json, os, sys, datetime
entry = {"signature": os.environ["SIG"], "url": os.environ["UPDATE_URL"]}
json.dump({
    "version": os.environ["VERSION"],
    "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "platforms": {"darwin-aarch64": entry, "darwin-x86_64": entry},
}, open(sys.argv[1], "w"), indent=2)
PY
UPLOAD_TAR="$(mktemp -d)/$TAR_NAME"
cp "$UPDATE_TAR" "$UPLOAD_TAR"

gh release create "$TAG" "$DMG_PATH" "$UPLOAD_TAR" "$LATEST_JSON" \
  --title "MixInspo DJ $TAG" \
  --target "$BRANCH" \
  --notes "$NOTES"

echo "==> Starting the Windows build on GitHub (adds the installer to $TAG when it finishes)"
if ! gh workflow run windows.yml -f tag="$TAG"; then
  echo "Couldn't start the Windows build. Run it by hand: GitHub -> Actions -> Windows build, tag $TAG." >&2
fi

echo ""
echo "==> Done. The landing page links to the latest release, so it needs no edit."
