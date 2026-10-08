#!/bin/sh

mkdir -p ~/.config/herdr/hooks
cp config.toml ~/.config/herdr/config.toml
cp hooks/tab-title.sh ~/.config/herdr/hooks/tab-title.sh

herdr integration install claude
herdr integration install codex
herdr integration install opencode

mkdir -p ~/.claude/skills/herdr ~/.agents/skills/herdr
herdr --skill > ~/.claude/skills/herdr/SKILL.md
herdr --skill > ~/.agents/skills/herdr/SKILL.md

herdr plugin install mrcndz/herdr-routines --ref cd504512d2f1976d39668fbdc0fe9b86cf3eff85
routines_dir="$(herdr plugin config-dir herdr-routines)"
mkdir -p "$routines_dir"
touch "$routines_dir/routines.toml"
