#!/usr/bin/env bash
# Marketplace-facing checks for the bundled backend.
#
# Reviewers bind approval to an exact SHA. At that SHA the committed ELF must
# be inspectable with `nm` (not stripped), its recorded hash must match the
# bytes in git, and a fresh pinned rebuild must be byte-identical. This script
# is the single gate used by CI and by the tag-release workflow so a later
# release cannot skip any of those attestations.
#
# Usage: scripts/verify-bundle.sh
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
target="x86_64-unknown-linux-musl"
bin="$repo_root/omarchy/bin/wireview-pro2-qs"
expected_file="$repo_root/omarchy/bin/wireview-pro2-qs.sha256"

fail() {
  echo "verify-bundle: $*" >&2
  exit 1
}

cd "$repo_root"

# --- inspectability and recorded identity (fast; no rebuild) ---

[[ -f "$bin" ]] || fail "missing committed ELF $bin"
[[ -f "$expected_file" ]] || fail "missing $expected_file"

expected="$(awk '{print $1}' "$expected_file")"
[[ ${#expected} -eq 64 ]] || fail "recorded hash in $expected_file is not a SHA-256"

committed="$(sha256sum "$bin" | awk '{print $1}')"
if [[ "$committed" != "$expected" ]]; then
  fail "committed ELF does not match the recorded hash
  recorded:  $expected
  committed: $committed
Run 'scripts/build-bundle.sh' and commit both the binary and the .sha256 file."
fi

file_out="$(file -b "$bin")"
[[ "$file_out" == ELF* ]] || fail "committed file is not an ELF: $file_out"
[[ "$file_out" == *"not stripped"* ]] || fail "committed ELF is stripped (marketplace review inspects symbols with nm): $file_out"

command -v nm >/dev/null || fail "nm is required to attest inspectability"
nm "$bin" >/dev/null 2>&1 || fail "nm cannot read the committed ELF"
# Avoid grep -q: with pipefail, an early match SIGPIPEs nm and looks like failure.
if ! nm "$bin" | grep 'wireview_pro2_qs' >/dev/null; then
  fail "committed ELF has no crate symbols; it is not inspectable against the tracked Rust source"
fi

if grep -Eq '^strip[[:space:]]*=[[:space:]]*(true|"symbols"|"all")' "$repo_root/Cargo.toml"; then
  fail "Cargo.toml must not fully strip the release binary (use strip = \"debuginfo\")"
fi
grep -q '^strip = "debuginfo"' "$repo_root/Cargo.toml" || fail "Cargo.toml [profile.release] must set strip = \"debuginfo\" so nm can inspect the bundle"

grep -q 'rustfmt' "$repo_root/rust-toolchain.toml" || fail "rust-toolchain.toml must pin rustfmt on this channel (installing it only on stable skips CI format, which used to skip this job)"
grep -q 'clippy' "$repo_root/rust-toolchain.toml" || fail "rust-toolchain.toml must pin clippy on this channel"

if awk '
  $0 == "  verify-bundle:" {in_job=1; next}
  in_job && /^  [A-Za-z0-9_-]+:/ {in_job=0}
  in_job && /^    needs:/ {exit 0}
  END {exit 1}
' "$repo_root/.github/workflows/ci.yml"; then
  fail "CI job verify-bundle must not use needs: (a failed format check must not skip this attestation)"
fi

crate_version="$(awk -F '"' '/^version = / {print $2; exit}' "$repo_root/Cargo.toml")"
manifest_version="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' "$repo_root/manifest.json")"
[[ "$crate_version" == "$manifest_version" ]] || fail "Cargo.toml version ($crate_version) != manifest.json version ($manifest_version)"

bin_version="$("$bin" --version | awk '{print $NF}')"
[[ "$bin_version" == "$crate_version" ]] || fail "committed ELF --version is $bin_version but Cargo.toml is $crate_version; rebuild the bundle after bumping the version"

if [[ "${GITHUB_REF_TYPE:-}" == tag ]]; then
  tag="${GITHUB_REF_NAME#v}"
  [[ "$crate_version" == "$tag" ]] || fail "git tag ${GITHUB_REF_NAME} does not match crate version $crate_version"
fi

if [[ "${VERIFY_BUNDLE_SKIP_REBUILD:-}" == 1 ]]; then
  echo "verified: omarchy/bin/wireview-pro2-qs is non-stripped, version $crate_version, hash $committed (rebuild skipped)"
  exit 0
fi

# --- byte-for-byte rebuild of the tracked source ---

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

export RUSTFLAGS="${RUSTFLAGS:-} \
  --remap-path-prefix=${cargo_home}/registry/src=./registry/src \
  --remap-path-prefix=${repo_root}=."
export CARGO_TARGET_DIR="$tmp/target"

cargo build --release --locked --target "$target"

actual="$(sha256sum "$tmp/target/$target/release/wireview-pro2-qs" | awk '{print $1}')"
if [[ "$expected" != "$actual" ]]; then
  fail "bundled binary does not match the reproducible build of the tracked source
  expected: $expected
  actual:   $actual
Run 'scripts/build-bundle.sh' to regenerate the bundle, then commit it."
fi

echo "verified: omarchy/bin/wireview-pro2-qs is non-stripped, version $crate_version, and matches the reproducible build ($actual)"
