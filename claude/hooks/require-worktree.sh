#!/bin/bash
input="$(cat)"
cwd="$(jq -r '.cwd // empty' <<<"$input")"
cd "${cwd:-$PWD}" 2>/dev/null || exit 0

if [[ "$(jq -r '.tool_name // empty' <<<"$input")" == "apply_patch" ]]; then
  paths="$(jq -r '.tool_input | if type == "string" then . else .command // empty end' <<<"$input" \
    | sed -nE -e 's/^\*\*\* (Add|Update|Delete) File: //p' -e 's/^\*\*\* Move to: //p')"
  next_step="use absolute paths in that worktree"
else
  paths="$(jq -r '.tool_input.file_path // .tool_input.notebook_path // empty' <<<"$input")"
  next_step="call EnterWorktree with its path"
fi

while IFS= read -r path; do
  [[ -n "$path" ]] || continue
  dir="$(dirname "$path")"
  until [[ -d "$dir" ]]; do dir="$(dirname "$dir")"; done
  git_dir="$(git -C "$dir" rev-parse --git-dir 2>/dev/null)" || continue
  [[ "$git_dir" == */worktrees/* ]] && continue
  echo "Do not edit the main checkout. Create a worktree in ~/.worktrees/<repo-name>/<slug> from origin/HEAD, then $next_step. See the Worktrees rules." >&2
  exit 2
done <<<"$paths"
exit 0
