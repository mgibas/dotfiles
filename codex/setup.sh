#!/bin/sh

mkdir -p ~/.codex
cp AGENTS.md ~/.codex/AGENTS.md
cp config.toml ~/.codex/config.toml
printf '\n[sandbox_workspace_write]\nwritable_roots = ["%s/.worktrees"]\n' "$HOME" >> ~/.codex/config.toml
cp hooks.json ~/.codex/hooks.json
