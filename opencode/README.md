# OpenCode session PRs

The OpenCode V2 sidebar shows your GitHub PRs registered to the active session. Each session has its own saved PR list. The list remains available after a restart.

Each row has a PR link, clickable title, colored review state, and check results. The panel shows up to five PRs and refreshes every 60 seconds. A failed lookup shows its error on that PR. Saved status is marked **Stale** when a refresh fails.

## Commands

| Command | Action |
| --- | --- |
| `/pr add <url>` | Validate and link one of your existing GitHub PRs to this session. |
| `/pr remove <url>` | Remove a link from this session. |
| `/pr list` | Show this session’s saved PR links. |
| `/pr` | Show command help. |

Without a URL, `/pr add` asks for one and `/pr remove` opens a selection list. Commands run locally in the terminal interface. They do not send a model prompt or change GitHub.

## Automatic registration

The plugin adds a PR automatically when a shell command in the session runs `gh pr create`, exits with code 0, and prints the PR URL. This includes agent commands and `!` shell commands. The plugin scans the session history when the sidebar opens, and it watches new messages.

The plugin ignores PR URLs in other output, such as `gh pr list` results or file contents. It adds only PRs that the signed-in GitHub user authored. Use `/pr add` for PRs created outside the session, for example in the GitHub web interface.

`/pr remove` stops automatic registration of that PR for the session. `/pr add` registers it again.

The plugin requires OpenCode V2 and an authenticated GitHub CLI (`gh auth status`).

Install from this directory:

```sh
sh setup.sh
```

The installer keeps existing CLI settings and plugins. Restart the OpenCode terminal interface after installation. The sidebar must be visible (`session.sidebar: "auto"` in `cli.json`).

Run the tests:

```sh
cd session-prs
node --test *.test.js
```
