#!/bin/bash
# Runs a command and fails if it changed the user's iceblink cache.
#
# Usage: scripts/check-user-cache.sh cargo test -p iceblink-wgpu
#
# Another process that uses the cache while the command runs fails the check too.
set -uo pipefail

cache="${XDG_CACHE_HOME:-$HOME/.cache}/iceblink"

list() {
    if [ -d "$cache" ]; then
        find "$cache" -exec stat -c '%n %s %Y' {} + | sort
    fi
}

before=$(list)
"$@"
status=$?
after=$(list)

if [ "$before" != "$after" ]; then
    echo "the command changed $cache:" >&2
    diff <(echo "$before") <(echo "$after") >&2
    exit 1
fi
exit "$status"
