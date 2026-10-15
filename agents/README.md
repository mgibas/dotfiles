# Shared skills

The three generic skills in `agents/skills/` are the source for Codex and Claude Code:
`show-me`, `pull-repos`, and `usage-review`.
Edit those files in this repository.

From the repository root, install the links:

```sh
(cd agents && sh ./setup.sh)
```

The Claude and Codex setup scripts also run this step.
Codex reads `~/.agents/skills/<name>`, which links to the skill folder in this
repository. Claude reads `~/.claude/skills/<name>`, which links to the same folder
through `~/.agents/skills/<name>`.

The installer backs up existing folders or other links under
`~/.agents/skill-backups/dotfiles-*`. It keeps custom skills, plugin skills, and
Claude-synced skills. Run it again after you add a skill to `agents/skills/`.
Keep this checkout at the same path while its links are in use.

Existing Claude script paths remain valid. Scheduled tasks can keep those paths.
Reload skills or restart the provider session if the skills do not appear.

Provider-specific tools, permissions, and history sources still apply.
`usage-review` reads Claude Code history.
