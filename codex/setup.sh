#!/bin/sh

mkdir -p ~/.codex
cp AGENTS.md ~/.codex/AGENTS.md
mkdir -p ~/.agents/skills
cp -R ../claude/skills/show-me ~/.agents/skills/
cp config.toml ~/.codex/config.toml
printf '\n[sandbox_workspace_write]\nwritable_roots = ["%s/.worktrees"]\n' "$HOME" >> ~/.codex/config.toml
cp hooks.json ~/.codex/hooks.json
