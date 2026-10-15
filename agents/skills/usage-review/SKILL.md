---
name: usage-review
description: Weekly review of the last 30 days of Claude Code sessions; suggests skills or context changes for repeated prompts and removals for unused setup
disable-model-invocation: true
allowed-tools: Bash(python3 -I "$HOME/.agents/skills/usage-review/collect.py" *), Read(~/.claude/**), Read(~/Projects/**), Write(~/.claude/usage-review/**)
---

You review how I used Claude Code in the last 30 days. You suggest changes to my setup. You do not change the setup yourself unless I approve each change in this session.

"No changes" is a valid and expected result. Do not invent a suggestion to fill the report.

## 1. Collect data

Run:

```bash
python3 -I "$HOME/.agents/skills/usage-review/collect.py" collect
```

The output is JSON:

- `window`: the last 30 days, up to today 00:00.
- `prompt_chunks`: JSON files with the sessions of the window, in time order, about 120 KB each. Each file maps a session ID to `cwd`, `title`, `started`, my typed `prompts`, my `commands` (slash commands with args), and `skills_invoked` (Skill tool calls by the model). Headless `claude -p` sessions and sessions of this skill are not included.
- `usage_last_days`: skill and slash command use over the window. `transcripts_available_since` tells you how far back data goes.
- `inventory`: user, synced, project and plugin skills, commands, agents, plugins, context files (CLAUDE.md, AGENTS.md, memory indexes), and `routines_toml`.
- `previous_reports`: my last reports. `report_path`: where you write this report.

If `session_count` is 0, write "No sessions in window. No changes." to `report_path`, show it, and stop.

## 2. Find repeated prompts

Do not read the chunk files yourself. They are too large for one context. Start one subagent for each item in `prompt_chunks`, all in one message so they run in parallel. Give each subagent its chunk path and these instructions:

> Read the file. It contains Claude Code sessions with the prompts that the user typed. Find instructions, corrections, procedures, and pasted context that the user repeats in two or more sessions. Paraphrases count as repeats. Return a list. For each item: a one-line description, a case-insensitive regex that matches the typical wording, the number of sessions, the projects (`cwd`), and two short verbatim quotes. Return "none" if you find no repeats. Do not suggest fixes.

Typical signals:

- The same constraint again and again ("use a worktree from latest main", "don't comment", "draft PR").
- A correction of model behavior ("no", "I told you", "again", "stop doing X").
- The same multi-step procedure typed out by hand.
- The same context pasted again (URLs, IDs, paths, team names).

Merge the subagent results. A pattern that occurs in more than one chunk is stronger evidence. Confirm the frequency of each merged candidate over the full window:

```bash
python3 -I "$HOME/.agents/skills/usage-review/collect.py" grep '<regex>'
```

Drop a candidate with fewer than 4 hits in 3 or more sessions, unless it is a correction of a rule that already exists. Note if a pattern stopped in the second half of the window: then the problem can be fixed already.

Before you suggest a fix, read the context files and skills in `inventory` that apply. If the rule already exists and I still had to repeat it, the problem is that the model ignores the rule. Then suggest a stronger or clearer wording, a better position, or a hook. Do not suggest a duplicate rule.

Pick the target for each fix:

| Pattern | Target |
|---|---|
| Rule I repeat in more than one repo | `~/.claude/CLAUDE.md` |
| Rule I repeat in one repo only | that repo's `CLAUDE.md` (if the team owns the file, say so; else `CLAUDE.local.md`) |
| Multi-step procedure, used in more than one repo | new user skill in `~/.agents/skills/`, linked from `~/.claude/skills/` |
| Multi-step procedure for one repo | new project skill in `<repo>/.claude/skills/` |
| Action that must happen every time, with no judgment (format, lint, block a command) | hook in `settings.json` |
| Fact about ongoing work | memory file |
| Change to an existing skill | edit that skill |

Repeated permission approvals are out of scope. If you see many, mention `/fewer-permission-prompts` in one line.

## 3. Find setup to remove

Use `usage_last_days` and `inventory`. Window data alone is not evidence of non-use.

Candidates:

- A user or project skill with 0 uses in the window. Before you suggest removal, check `routines_toml`: a skill that a routine runs is in use, even if the routine is disabled (then ask if the routine is still needed). A skill with `model_invocable: true` that never triggers can have a bad description; say which case applies.
- A plugin that is enabled with 0 skill uses in 30 days. Note that MCP tool use is not in this data, so look for its MCP tool names in the transcripts (`grep -l 'mcp__<server>' ~/.claude/projects/*/*.jsonl | wc -l`) before you suggest removal.
- Two skills, or a skill and a plugin skill, that do the same job.
- A rule in a context file that conflicts with another rule, duplicates a skill, or names a tool, file, or project that no longer exists.
- A memory entry about a project that has ended, or one that the repo already records.

Do not suggest to remove synced managed skills (`user (synced, managed)`). Do not suggest to remove built-in commands.

If `transcripts_available_since` is less than 30 days ago, say so and treat 0-use results as weak.

## 4. Check previous reports

Read the last 2 `previous_reports`. Do not repeat a suggestion that I rejected. If a suggestion is still open and the evidence grew, list it as "still open" with the new count.

## 5. Report

Write the report to `report_path` and show the same text here. Use ASD-STE100 Simplified Technical English. Format:

```markdown
# Usage review YYYY-MM-DD

Window: <start> to <end>. Sessions: N. Prompts: N.

## Optimize

1. <short title> — <target file or "new skill <name>">
   - Evidence: <count> hits in <count> sessions over 30 days. Example: "<short quote>"
   - Change: <the exact rule text, or the skill name, description, and steps>
   - Why here: <why this target and not another>

## Remove

1. <item> — <path>
   - Evidence: <uses in 30 days, last use, routine reference>
   - Risk: <what breaks if this is wrong>

## Still open

- <item from a previous report, new evidence>
```

Write "None." under a section with no items. If both sections are empty, the first line after the window is "No changes suggested."

Keep each item short. Maximum 5 items in each section, strongest evidence first.

## 6. Apply

After the report, ask me which items to apply. Apply only the items that I approve. Then add two lines to the end of the report file: "Applied: <item numbers>" and "Rejected: <item numbers>". Step 4 of the next run uses these lines.
