# Communication Style

- Respond using ASD-STE100 Simplified Technical English
- Be direct and critical. Challenge my ideas, assumptions, and proposed solutions — don't just go along with them.
- If my reasoning has a flaw, say so plainly. If my approach is suboptimal, propose a better one and explain why.
- Validate my statements against evidence before accepting them. Don't treat my claims as given.
- Skip pleasantries, affirmations, and softening language. No "Great question!", no "That's a good point!", no hedging.
- Disagreement is expected and welcome. Be cold, calculated, and precise.

# Personal Preferences

## Git

- Use `mg/` prefix for any git branches created (e.g., `mg/feature-name`) unless the project or user specifies otherwise
- Do not include "Co-Authored-By: Claude" in commits or PRs
- Use conventional commit format for commit messages: single line, no description (e.g., `feat: add user authentication`)
- Always create PRs as drafts
- Don't add detailed description to commit message - leave it for PR description.
- PR description format:

```markdown
## Summary

Short description of what changed, no unnecessary fluff.
Concise bullet list with key high level changes; code provides details.
Write in ASD-STE100 Simplified Technical English.

Include screenshots in the PR description when the change is visual. Attach them with
`gh pr edit <n> --attach ./login.png` and reference each one in the body as
`![Updated login screen](./login.png)` — the reference is rewritten to the uploaded URL.
Without `--attach` the link is dead.

- One screenshot when the change is an addition — there is no "before".
- Before and after in every other case, including when the claim is that nothing changed
  visually. One image cannot show that, and a pair catches the differences you did not expect.

If user provided Linear issue at any point in time, reference it:

ref TICKET-ID - if it's just related
closes TICKET-ID - if it resolves provided ticket
```

- Always ask user before commit, push or PR create operations

### Final State Only

- The deliverable is the diff against the merge base, not the path we took to it.
- Before you write a commit message, a PR description, or tests, read the actual diff (`git diff <base>...HEAD`). Describe only what that diff contains.
- Do not mention rejected approaches, intermediate solutions, removed code, or the order of our iterations. If it is not in the final diff, it does not exist.
- Do not write text of the form "changed X to Y", "no longer does Z", "replaced the previous approach". Describe the end state.
- Tests must test the code that exists. Delete tests for behavior we removed. Do not keep a test as proof that an old approach failed.
- Exception: a real behavior change for the user or for a caller of a public API. Then state the new behavior and the migration step, not our design journey.

### Worktrees

- Do each code change in a git repo in a worktree for that task. Do not edit files in the main checkout.
- If the session already runs in a worktree for this task, use it. If the task continues an existing branch, use the worktree of that branch, or create one for it.
- Otherwise, before the first edit, run `git -C <repo> fetch origin` and `git -C <repo> worktree add -b mg/<ticket-id>-<slug> ~/.worktrees/<repo-name>/<slug> origin/HEAD`. If there is no ticket, use `mg/<slug>`.
- Then call `EnterWorktree` with the worktree path. Do not edit worktree files through absolute paths from the main checkout.
- Do not use a worktree of a different task.

## GitHub CLI

- Prefer dedicated `gh` subcommands over `gh api` for read operations; use `gh api` only when no subcommand exists (e.g., reading PR review comments)
- Never post comments, reviews, or any other content to GitHub PRs/issues — including via `gh pr comment`, `gh issue comment`, `gh pr review`, or `gh api`-based mutations to /comments or /reviews endpoints. If you think a GitHub comment is the right move, ask first.

## VS Code

- When I ask to open VS Code, run `code -n <root of the worktree that holds the changes>`. Do not use `code .` and do not open single files unless I name them.

## Code Comments

- Do not write comments. Make the code self-explanatory with names, types, and small functions.
- Add a comment only if all of these are true: the reason for the code is not visible in the code, the reader can break the code without it, and no rename or refactor can remove the need.
- Permitted comments: a non-obvious "why" (workaround, spec/protocol rule, performance trade-off, known bug reference), a required annotation (`@ts-expect-error`, `eslint-disable`, license header), or an API doc block that the project already uses on public exports.
- Forbidden comments: a restatement of the code, a section marker, a step-by-step narration, a note about what changed or what was there before, a "removed X" or "no longer uses Y" note, and commented-out code.
- Do not add a comment to explain a change to me. Put that in the chat or the PR description.
- Keep the comment density of the file you edit. If the file has no comments, add none.
- If you delete code, delete its comments too.

## Tone and voice

- Use ASD-STE100 Simplified Technical English for any prose content including, but not limited to: PR descriptions, GitHub comments, code comments, documentation, briefs.
- Be concise, don't get into details that can be found somewhere else

## Browser Tooling

- Use chrome-devtools MCP for all browser work: visual verification (e.g. checking rendered Storybook stories, confirming UI changes), automation (navigate, click, fill forms, read the page), and inspection (performance traces, network requests, console debugging, heap snapshots).
- Do not use the `preview_*` tools. Playwright is not installed.
- chrome-devtools does not start a dev server. Start it in a separate Herdr pane when `HERDR_ENV=1` (see below), otherwise as a background Bash command, then navigate to its URL.

# Agent Environment: Herdr Multiplexer
When `HERDR_ENV=1` is set, load the `herdr` skill before you start a dev server, a long test run, or a parallel agent. Run it in a sibling pane, not as a background Bash command. This rule applies when I do not mention Herdr.
