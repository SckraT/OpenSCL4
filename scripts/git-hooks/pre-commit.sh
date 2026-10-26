#!/usr/bin/env bash
# Fast checks for the files staged in this commit. CI still runs the full gate (docs/development.md).
set -euo pipefail

staged=()
while IFS= read -r file; do
    staged+=("$file")
done < <(git diff --cached --name-only --diff-filter=ACMR)
if [ "${#staged[@]}" -eq 0 ]; then
    exit 0
fi

failed=0

# Prints the staged files whose path matches an extended regular expression.
staged_matching() {
    printf '%s\n' "${staged[@]}" | grep -E "$1" || true
}

skip() {
    echo "pre-commit: $1 is not installed, skipping $2; CI still checks it" >&2
}

if [ -n "$(staged_matching '\.rs$')" ]; then
    cargo fmt --all --check || failed=1
fi

kotlin=()
while IFS= read -r file; do
    kotlin+=("$file")
done < <(staged_matching '^android/.*\.kts?$')
if [ "${#kotlin[@]}" -gt 0 ]; then
    if command -v ktlint > /dev/null; then
        ktlint --editorconfig=.editorconfig "${kotlin[@]}" || failed=1
    else
        skip ktlint "Kotlin formatting"
    fi
fi

# The Markdown files `just format-check` passes to Prettier.
markdown=()
while IFS= read -r file; do
    markdown+=("$file")
done < <(staged_matching '^(README\.md|CHANGELOG\.md|docs/[^/]+\.md|tools/soundcore-device-faker/README\.md)$')
if [ "${#markdown[@]}" -gt 0 ]; then
    if command -v prettier > /dev/null; then
        # A checkout with core.autocrlf has CRLF files; CI checks the LF blobs.
        prettier --check --end-of-line auto "${markdown[@]}" || failed=1
    else
        skip prettier "Markdown formatting"
    fi
fi

shell=()
while IFS= read -r file; do
    shell+=("$file")
done < <(staged_matching '\.sh$')
if [ "${#shell[@]}" -gt 0 ]; then
    if command -v shellcheck > /dev/null; then
        shellcheck "${shell[@]}" || failed=1
    else
        skip shellcheck "shell scripts"
    fi
fi

# Inputs of scripts/tests: the scripts, workflows, README and the device catalog.
if [ -n "$(staged_matching '^(scripts/|\.github/workflows/|README\.md(\.in)?$|lib/src/devices/device_model\.rs$|lib/i18n/en/)')" ]; then
    # On Windows `python3` may be the Microsoft Store stub, which exits without running anything.
    python=
    for candidate in python3 python; do
        if command -v "$candidate" > /dev/null && "$candidate" -c '' 2> /dev/null; then
            python=$candidate
            break
        fi
    done
    if [ -n "$python" ]; then
        "$python" -m unittest discover -s scripts/tests || failed=1
    else
        skip python "scripts/tests"
    fi
fi

if [ "$failed" -ne 0 ]; then
    echo "pre-commit: fix the problems above, or skip the hook once with git commit --no-verify" >&2
    exit 1
fi
