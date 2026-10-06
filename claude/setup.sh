#!/bin/sh

mkdir -p ~/.claude
cp -R skills ~/.claude/
cp -R hooks ~/.claude/
cp CLAUDE.md ~/.claude/CLAUDE.md
mkdir -p ~/.worktrees
jq --arg dir "$HOME/.worktrees" '.permissions.additionalDirectories = [$dir]' settings.json > ~/.claude/settings.json
cp status-line.sh ~/.claude/status-line.sh
cp -R output-styles ~/.claude/
claude mcp add --scope user chrome-devtools -- npx chrome-devtools-mcp@latest --autoConnect
claude mcp add --scope user --transport http figma https://mcp.figma.com/mcp
