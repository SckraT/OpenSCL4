#!/usr/bin/env bash
# Installs a pre-commit hook that runs scripts/git-hooks/pre-commit.sh. The hooks directory is shared by all
# worktrees of a clone, so the hook runs the script of the worktree being committed and does nothing where the
# script does not exist. Other hooks in the directory, such as Memorix's post-commit, stay as they are.
set -euo pipefail

marker='# [openscl4-pre-commit]'
hooks=$(git rev-parse --path-format=absolute --git-path hooks)
target="$hooks/pre-commit"

if [ -e "$target" ] && ! grep -qF "$marker" "$target"; then
    echo "$target already exists and is not the OpenSCL4 hook; remove it or call scripts/git-hooks/pre-commit.sh from it" >&2
    exit 1
fi

mkdir -p "$hooks"
cat > "$target" << EOF
#!/bin/sh
$marker installed by scripts/git-hooks/install.sh
hook=scripts/git-hooks/pre-commit.sh
[ -f "\$hook" ] || exit 0
exec bash "\$hook"
EOF
chmod +x "$target"
echo "Installed $target"
