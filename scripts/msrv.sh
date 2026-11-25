#!/bin/bash
# Checks the workspace on the minimum supported Rust version, the `rust-version` in Cargo.toml.
#
# Usage: scripts/msrv.sh [cargo check arguments]
set -euo pipefail

cd "$(dirname "$0")/.."

version=$(sed -n 's/^rust-version = "\(.*\)"$/\1/p' Cargo.toml)
if [ -z "$version" ]; then
    echo "no rust-version in Cargo.toml" >&2
    exit 1
fi

# `+1.90` would pick the latest 1.90 patch release, and a `rust-version` of 1.90 means 1.90.0.
case "$version" in
    *.*.*) ;;
    *) version="$version.0" ;;
esac

exec cargo "+$version" check --workspace --all-targets --all-features "$@"
