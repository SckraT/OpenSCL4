#!/usr/bin/env bash
set -euo pipefail

script_path="$(readlink -f -- "$0")"
script_dir="$(dirname -- "$script_path")"
awk '$1 == "ndk" { print $2 }' "$script_dir/../../.build-tool-versions"

