#!/usr/bin/env bash
#
# .script-deploy.sh — one-shot promotion of a Memryzed GitHub Release to
# the live download endpoint (memryzed.com).
#
# This is the manual "promote" half of the release model: CI builds,
# smoke-tests, attests, and publishes the GitHub Release. This script,
# run once from a trusted machine, downloads those exact artifacts,
# verifies their checksums, uploads them to S3, points latest.txt at the
# new version, invalidates the CDN, and verifies the live install.
#
# It is idempotent: re-running re-uploads the same files and re-points
# latest.txt. Nothing here builds binaries.
#
# Usage:
#   ./.script-deploy.sh                 # deploy the version in Cargo.toml
#   ./.script-deploy.sh v0.7.1          # deploy a specific tag
#   ./.script-deploy.sh v0.7.1 --no-latest    # upload but don't flip latest.txt
#   DRY_RUN=1 ./.script-deploy.sh       # print actions without changing anything
#
# Requirements: gh (authenticated), aws (profile below), tar, sha256sum.

set -euo pipefail

# --- Configuration (override via environment) --------------------------
REPO="${MEMRYZED_REPO:-memryzed/memryzed}"
BUCKET="${MEMRYZED_BUCKET:-memryzed.com}"
DISTRIBUTION_ID="${MEMRYZED_CF_DISTRIBUTION:-E2IBTNPP9XL1J9}"
AWS_PROFILE_NAME="${MEMRYZED_AWS_PROFILE:-cloney}"
AWS_REGION_NAME="${MEMRYZED_AWS_REGION:-us-east-1}"
EXPECTED_TARGETS=(
  "memryzed-x86_64-unknown-linux-gnu.tar.gz"
  "memryzed-aarch64-unknown-linux-gnu.tar.gz"
  "memryzed-aarch64-apple-darwin.tar.gz"
  "memryzed-x86_64-pc-windows-msvc.zip"
)
# -----------------------------------------------------------------------

DRY_RUN="${DRY_RUN:-0}"
FLIP_LATEST=1
TAG=""

for arg in "$@"; do
  case "$arg" in
    --no-latest) FLIP_LATEST=0 ;;
    v*) TAG="$arg" ;;
    *) echo "error: unknown argument: $arg" >&2; exit 2 ;;
  esac
done

say()  { printf '\033[0;36m==>\033[0m %s\n' "$*"; }
ok()   { printf '\033[0;32m  ok\033[0m %s\n' "$*"; }
die()  { printf '\033[0;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

run() {
  # Echo and execute, or just echo under DRY_RUN.
  if [ "$DRY_RUN" = "1" ]; then
    printf '\033[0;33m  [dry-run]\033[0m %s\n' "$*"
  else
    "$@"
  fi
}

aws_() { aws "$@" --profile "$AWS_PROFILE_NAME" --region "$AWS_REGION_NAME"; }

# --- Preflight ---------------------------------------------------------
command -v gh  >/dev/null || die "gh not found"
command -v aws >/dev/null || die "aws not found"
command -v sha256sum >/dev/null || die "sha256sum not found"

cd "$(dirname "$0")"

if [ -z "$TAG" ]; then
  ver="$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')"
  [ -n "$ver" ] || die "could not read version from Cargo.toml"
  TAG="v${ver}"
fi
VERSION="${TAG#v}"

say "Deploying $TAG  (repo=$REPO  bucket=$BUCKET  profile=$AWS_PROFILE_NAME)"
[ "$DRY_RUN" = "1" ] && say "DRY RUN — no changes will be made"

gh auth status >/dev/null 2>&1 || die "gh is not authenticated (run: gh auth login)"
aws_ sts get-caller-identity >/dev/null 2>&1 || die "AWS profile '$AWS_PROFILE_NAME' cannot authenticate"

gh release view "$TAG" --repo "$REPO" >/dev/null 2>&1 \
  || die "GitHub release $TAG not found in $REPO (CI must publish it first)"

# --- Download release artifacts ----------------------------------------
workdir="$(mktemp -d)"
trap 'rm -rf "$workdir"' EXIT
say "Downloading $TAG artifacts from GitHub Release"
gh release download "$TAG" --repo "$REPO" --pattern "*" --dir "$workdir" \
  || die "failed to download release assets"

# --- Verify every expected target is present and checksums match -------
say "Verifying artifacts"
for archive in "${EXPECTED_TARGETS[@]}"; do
  [ -f "$workdir/$archive" ]        || die "missing expected artifact: $archive"
  [ -f "$workdir/$archive.sha256" ] || die "missing checksum: $archive.sha256"
  expected="$(awk '{print $1}' "$workdir/$archive.sha256")"
  actual="$(sha256sum "$workdir/$archive" | awk '{print $1}')"
  [ "$expected" = "$actual" ] || die "checksum mismatch for $archive"
  ok "$archive"
done

# --- Upload to S3 ------------------------------------------------------
content_type() {
  case "$1" in
    *.tar.gz) echo "application/gzip" ;;
    *.zip)    echo "application/zip" ;;
    *.sha256) echo "text/plain" ;;
    *)        echo "application/octet-stream" ;;
  esac
}

say "Uploading to s3://$BUCKET/releases/$TAG/"
for archive in "${EXPECTED_TARGETS[@]}"; do
  for f in "$archive" "$archive.sha256"; do
    run aws_ s3 cp "$workdir/$f" "s3://$BUCKET/releases/$TAG/$f" \
      --content-type "$(content_type "$f")"
  done
done
ok "all artifacts uploaded"

# --- Flip latest.txt ---------------------------------------------------
if [ "$FLIP_LATEST" = "1" ]; then
  say "Pointing latest.txt at $VERSION"
  printf '%s\n' "$VERSION" > "$workdir/latest.txt"
  run aws_ s3 cp "$workdir/latest.txt" "s3://$BUCKET/releases/latest.txt" \
    --content-type "text/plain" --cache-control "no-cache, max-age=0"
  ok "latest.txt -> $VERSION"
else
  say "Skipping latest.txt (--no-latest); existing installs unaffected"
fi

# --- Sync install scripts (kept in dist/) ------------------------------
if [ -d dist ]; then
  say "Syncing install scripts from dist/"
  for s in install.sh install.ps1 install.cmd; do
    [ -f "dist/$s" ] || continue
    ct="text/x-shellscript"; [ "$s" != "install.sh" ] && ct="text/plain"
    run aws_ s3 cp "dist/$s" "s3://$BUCKET/$s" \
      --content-type "$ct" --cache-control "no-cache, max-age=0"
  done
  ok "install scripts synced"
fi

# --- Invalidate CDN ----------------------------------------------------
say "Invalidating CloudFront"
run aws_ cloudfront create-invalidation --distribution-id "$DISTRIBUTION_ID" \
  --paths "/releases/*" "/install.sh" "/install.ps1" "/install.cmd" \
  --query 'Invalidation.{Id:Id,Status:Status}' --output json

# --- Verify live -------------------------------------------------------
if [ "$DRY_RUN" = "1" ]; then
  say "Dry run complete. No changes were made."
  exit 0
fi

say "Verifying live endpoint (allowing for CDN propagation)"
sleep 15
live_latest="$(curl -fsSL "https://$BUCKET/releases/latest.txt" | tr -d '[:space:]' || true)"
if [ "$FLIP_LATEST" = "1" ] && [ "$live_latest" != "$VERSION" ]; then
  say "latest.txt still shows '$live_latest' at the edge; invalidation may need another minute"
else
  ok "latest.txt = $live_latest"
fi
for archive in "${EXPECTED_TARGETS[@]}"; do
  code="$(curl -s -o /dev/null -w '%{http_code}' -I "https://$BUCKET/releases/$TAG/$archive")"
  [ "$code" = "200" ] && ok "200 $archive" || say "WARN: $code for $archive (CDN may still be propagating)"
done

say "Deploy of $TAG complete."
