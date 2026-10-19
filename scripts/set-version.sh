#!/usr/bin/env bash
set -euo pipefail

script_dir="$(dirname "$(readlink -f -- "$0")")"
exec python3 "$script_dir/release.py" set-version "${1:?Usage: set-version.sh major.minor.patch}"
