#!/bin/sh

(
  set -eu

  skills_dir="$(pwd -P)/skills"
  backup_dir=
  mkdir -p "$HOME/.agents/skills" "$HOME/.claude/skills"

  link_skill() {
    if [ -L "$2" ] && [ "$(readlink "$2")" = "$1" ]; then
      return
    fi
    if [ -e "$2" ] || [ -L "$2" ]; then
      if [ -z "$backup_dir" ]; then
        mkdir -p "$HOME/.agents/skill-backups"
        backup_dir="$(mktemp -d "$HOME/.agents/skill-backups/dotfiles-XXXXXXXX")"
      fi
      mkdir -p "$backup_dir/$(dirname "$3")"
      mv "$2" "$backup_dir/$3"
    fi
    ln -s "$1" "$2"
  }

  for skill_dir in "$skills_dir"/*; do
    [ -f "$skill_dir/SKILL.md" ] || continue
    skill_name="$(basename "$skill_dir")"
    link_skill "$skill_dir" "$HOME/.agents/skills/$skill_name" "agents/$skill_name"
    link_skill "$HOME/.agents/skills/$skill_name" "$HOME/.claude/skills/$skill_name" "claude/$skill_name"
  done

  [ -z "$backup_dir" ] || printf 'Skill backups: %s\n' "$backup_dir"
)
