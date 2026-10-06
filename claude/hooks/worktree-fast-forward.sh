#!/bin/bash
git rev-parse --git-dir 2>/dev/null | grep -q /worktrees/ || exit 0
git fetch origin --quiet 2>/dev/null || exit 0
ref=$(git symbolic-ref -q --short refs/remotes/origin/HEAD)
[ -n "$ref" ] || { git rev-parse -q --verify origin/main >/dev/null 2>&1 && ref=origin/main || ref=origin/master; }
before=$(git rev-parse HEAD)
git merge --ff-only --quiet "$ref" 2>/dev/null || exit 0
[ "$before" != "$(git rev-parse HEAD)" ] && echo "Worktree fast-forwarded to $ref ($(git log -1 --format=%h))"
exit 0
