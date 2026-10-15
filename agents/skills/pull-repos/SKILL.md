---
name: pull-repos
description: Refresh Git repositories in a directory with parallel fast-forward pulls. Use when the user asks to pull or update multiple local repositories.
---

# Pull repositories

Run the bundled Rust CLI. It requires `git` and `rust-script`.

Always pass the directory requested by the user with `--root`. For example:

```sh
rust-script "$HOME/.agents/skills/pull-repos/scripts/pull_repos.rs" --root "$HOME/Projects"
```

Use `--dry-run` for local checks without a fetch or merge. Use `--help` for all options. Defaults: 4 parallel jobs, 120 seconds per phase, search depth 2.

The script updates clean checkouts on their default branch. It skips linked worktrees and unsafe checkouts. It does not reset, stash, or force changes.

Read each repository result and report failures or skips. A dry run does not check for remote updates.

The default root is `~/Projects`. Use `--root` to select another directory.
