#!/bin/bash
# Runs the iceblink-alloc tests under miri, which needs a nightly toolchain.
#
# Usage: scripts/miri.sh [cargo miri test arguments]
set -euo pipefail

cd "$(dirname "$0")/.."
exec cargo +nightly-2026-04-16 miri test -p iceblink-alloc "$@"
