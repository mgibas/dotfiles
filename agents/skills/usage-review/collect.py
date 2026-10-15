#!/usr/bin/env python3
import argparse
import json
import re
import sys
import time
from collections import Counter, defaultdict
from datetime import datetime, timedelta
from pathlib import Path

HOME = Path.home()
CLAUDE = HOME / ".claude"
PROJECTS = CLAUDE / "projects"
REPORTS = CLAUDE / "usage-review"
ROUTINES = HOME / ".config/herdr/plugins/config/herdr-routines/routines.toml"
SELF_COMMAND = "usage-review"
PROMPT_LIMIT = 1500
WINDOW_DAYS = 30
CHUNK_BYTES = 120_000
HEADLESS_ENTRYPOINTS = {"sdk-cli", "sdk-ts", "sdk-py"}

COMMAND_NAME = re.compile(r"<command-name>/?([^<]+)</command-name>")
COMMAND_ARGS = re.compile(r"<command-args>(.*?)</command-args>", re.S)
SYSTEM_REMINDER = re.compile(r"<system-reminder>.*?</system-reminder>", re.S)
NOISE_PREFIXES = (
    "<local-command-stdout>",
    "<local-command-stderr>",
    "<local-command-caveat>",
    "<bash-stdout>",
    "<bash-stderr>",
    "<task-notification>",
    "[Request interrupted",
    "Caveat:",
)


def parse_ts(value):
    if not value:
        return None
    return datetime.fromisoformat(value.replace("Z", "+00:00")).astimezone()


def window(today, days):
    end = datetime.combine(today, datetime.min.time()).astimezone()
    return end - timedelta(days=days), end


def entrypoint(path):
    for entry in iter_entries(path, prefilter=('"entrypoint"',)):
        return entry.get("entrypoint")


def transcript_files(since):
    cutoff = since.timestamp()
    for path in PROJECTS.glob("*/*.jsonl"):
        if path.stat().st_mtime >= cutoff and entrypoint(path) not in HEADLESS_ENTRYPOINTS:
            yield path


def iter_entries(path, prefilter=None):
    with path.open(errors="replace") as handle:
        for line in handle:
            if prefilter and not any(token in line for token in prefilter):
                continue
            try:
                yield json.loads(line)
            except json.JSONDecodeError:
                continue


def human_text(entry):
    if entry.get("type") != "user" or entry.get("isMeta") or entry.get("isCompactSummary"):
        return None
    content = (entry.get("message") or {}).get("content")
    if isinstance(content, list):
        if any(block.get("type") == "tool_result" for block in content if isinstance(block, dict)):
            return None
        content = "\n".join(
            block.get("text", "") for block in content if isinstance(block, dict) and block.get("type") == "text"
        )
    if not isinstance(content, str):
        return None
    text = SYSTEM_REMINDER.sub("", content).strip()
    if not text or text.startswith(NOISE_PREFIXES):
        return None
    return text


def parse_command(text):
    name = COMMAND_NAME.search(text)
    if not name:
        return None
    args = COMMAND_ARGS.search(text)
    return name.group(1).strip(), (args.group(1).strip() if args else "")


def collect_window(start, end):
    sessions = {}
    for path in transcript_files(start):
        prompts, commands, skills, cwd, title = [], [], Counter(), None, None
        for entry in iter_entries(path):
            if entry.get("type") == "ai-title":
                title = entry.get("aiTitle") or entry.get("title") or title
            ts = parse_ts(entry.get("timestamp"))
            if not ts or not (start <= ts < end):
                continue
            cwd = entry.get("cwd") or cwd
            if entry.get("type") == "assistant":
                for block in (entry.get("message") or {}).get("content") or []:
                    if isinstance(block, dict) and block.get("type") == "tool_use" and block.get("name") == "Skill":
                        skills[(block.get("input") or {}).get("skill")] += 1
                continue
            text = human_text(entry)
            if text is None:
                continue
            command = parse_command(text)
            if command:
                commands.append({"name": command[0], "args": command[1][:PROMPT_LIMIT], "at": ts.strftime("%Y-%m-%d %H:%M")})
                continue
            if text.startswith("<bash-input>"):
                text = "!" + text.removeprefix("<bash-input>").removesuffix("</bash-input>")
            clipped = text if len(text) <= PROMPT_LIMIT else text[:PROMPT_LIMIT] + f" …[+{len(text) - PROMPT_LIMIT} chars]"
            prompts.append({"at": ts.strftime("%Y-%m-%d %H:%M"), "text": clipped})
        if any(command["name"] == SELF_COMMAND for command in commands):
            continue
        if prompts or commands:
            sessions[path.stem] = {
                "cwd": cwd,
                "title": title,
                "started": min(item["at"] for item in prompts + commands),
                "transcript": str(path),
                "prompts": prompts,
                "commands": commands,
                "skills_invoked": dict(skills),
            }
    return sessions


def collect_usage(days):
    since = datetime.now().astimezone() - timedelta(days=days)
    usage = defaultdict(lambda: {"skill_tool": 0, "slash": 0, "sessions": set(), "last": None})
    oldest = None
    for path in transcript_files(since):
        for entry in iter_entries(path, prefilter=('"Skill"', "<command-name>")):
            ts = parse_ts(entry.get("timestamp"))
            if not ts or ts < since:
                continue
            oldest = min(oldest, ts) if oldest else ts
            names = []
            if entry.get("type") == "assistant":
                for block in (entry.get("message") or {}).get("content") or []:
                    if isinstance(block, dict) and block.get("type") == "tool_use" and block.get("name") == "Skill":
                        names.append(("skill_tool", (block.get("input") or {}).get("skill")))
            else:
                text = human_text(entry)
                command = parse_command(text) if text else None
                if command:
                    names.append(("slash", command[0]))
            for kind, name in names:
                if not name:
                    continue
                record = usage[name]
                record[kind] += 1
                record["sessions"].add(path.stem)
                record["last"] = max(record["last"], ts) if record["last"] else ts
    history_start = min((p.stat().st_mtime for p in PROJECTS.glob("*/*.jsonl")), default=None)
    return {
        "days": days,
        "transcripts_available_since": time.strftime("%Y-%m-%d", time.localtime(history_start)) if history_start else None,
        "first_usage_event_seen": oldest.strftime("%Y-%m-%d") if oldest else None,
        "by_name": {
            name: {
                "skill_tool": record["skill_tool"],
                "slash": record["slash"],
                "sessions": len(record["sessions"]),
                "last": record["last"].strftime("%Y-%m-%d"),
            }
            for name, record in sorted(usage.items(), key=lambda item: -len(item[1]["sessions"]))
        },
    }


def frontmatter(path):
    try:
        text = path.read_text(errors="replace")
    except OSError:
        return {}
    match = re.match(r"---\n(.*?)\n---", text, re.S)
    fields = {}
    if match:
        for line in match.group(1).splitlines():
            if ":" in line and not line.startswith((" ", "\t")):
                key, value = line.split(":", 1)
                fields[key.strip()] = value.strip()
    fields["lines"] = text.count("\n")
    return fields


def describe_skill(skill_file, scope, prefix=""):
    meta = frontmatter(skill_file)
    return {
        "name": prefix + (meta.get("name") or skill_file.parent.name),
        "scope": scope,
        "path": str(skill_file),
        "description": meta.get("description", "")[:300],
        "model_invocable": meta.get("disable-model-invocation", "false") != "true",
        "lines": meta.get("lines"),
        "modified": time.strftime("%Y-%m-%d", time.localtime(skill_file.stat().st_mtime)),
    }


def plugin_inventory():
    settings = json.loads((CLAUDE / "settings.json").read_text())
    enabled = settings.get("enabledPlugins") or {}
    installed = json.loads((CLAUDE / "plugins/installed_plugins.json").read_text()).get("plugins", {})
    plugins, skills = [], []
    for plugin_id, installs in installed.items():
        user_install = next((i for i in installs if i.get("scope") == "user"), None)
        plugins.append({"id": plugin_id, "enabled": enabled.get(plugin_id, False), "user_scope": bool(user_install)})
        if user_install and enabled.get(plugin_id):
            prefix = plugin_id.split("@")[0] + ":"
            for skill_file in Path(user_install["installPath"]).glob("skills/*/SKILL.md"):
                skills.append(describe_skill(skill_file, f"plugin {plugin_id}", prefix))
    return plugins, skills


def project_root(cwd):
    path = Path(cwd)
    if ".claude/worktrees" in cwd:
        path = Path(cwd.split("/.claude/worktrees")[0])
    for parent in [path, *path.parents]:
        if parent.parent.name.endswith("-worktrees"):
            path = parent.parent.with_name(parent.parent.name.removesuffix("-worktrees"))
            break
    for candidate in [path, *path.parents]:
        if (candidate / ".git").exists():
            return candidate
        if candidate == HOME:
            break
    return path


def context_files(roots):
    files = [CLAUDE / "CLAUDE.md", HOME / "Projects/CLAUDE.md"]
    for root in roots:
        files += [root / "CLAUDE.md", root / "CLAUDE.local.md", root / ".claude/CLAUDE.md", root / "AGENTS.md"]
    memory = sorted(PROJECTS.glob("*/memory/MEMORY.md"))
    found = []
    for path in dict.fromkeys(files + memory):
        if path.exists():
            found.append({"path": str(path), "lines": path.read_text(errors="replace").count("\n")})
    return found


def inventory(sessions):
    roots = sorted({project_root(s["cwd"]) for s in sessions.values() if s["cwd"]} - {HOME, CLAUDE})
    skills = [describe_skill(f, "user") for f in sorted((CLAUDE / "skills").glob("*/SKILL.md"))]
    skills += [describe_skill(f, "user (synced, managed)") for f in sorted((CLAUDE / "skills/synced").glob("*/*/SKILL.md"))]
    for root in roots:
        skills += [describe_skill(f, f"project {root}") for f in sorted(root.glob(".claude/skills/*/SKILL.md"))]
    commands = [str(p) for p in sorted((CLAUDE / "commands").glob("*.md"))]
    for root in roots:
        commands += [str(p) for p in sorted(root.glob(".claude/commands/*.md"))]
    plugins, plugin_skills = plugin_inventory()
    return {
        "projects_touched": [str(r) for r in roots],
        "skills": skills + plugin_skills,
        "commands": commands,
        "agents": [str(p) for p in sorted((CLAUDE / "agents").glob("*.md"))],
        "plugins": plugins,
        "context_files": context_files(roots),
        "routines_file": str(ROUTINES),
        "routines_toml": ROUTINES.read_text() if ROUTINES.exists() else None,
    }


def previous_reports(limit=10):
    if not REPORTS.exists():
        return []
    return [str(p) for p in sorted(REPORTS.glob("*.md"))[-limit:]]


def write_chunks(sessions, directory):
    for old in directory.parent.glob("*/chunk-*.json"):
        old.unlink()
    directory.mkdir(parents=True, exist_ok=True)
    chunks, current, size = [], {}, 0
    for session_id, session in sorted(sessions.items(), key=lambda item: item[1]["started"]):
        encoded = len(json.dumps(session))
        if current and size + encoded > CHUNK_BYTES:
            chunks.append(current)
            current, size = {}, 0
        current[session_id] = session
        size += encoded
    if current:
        chunks.append(current)
    paths = []
    for index, chunk in enumerate(chunks, 1):
        path = directory / f"chunk-{index}.json"
        path.write_text(json.dumps(chunk, indent=1))
        first, last = min(s["started"] for s in chunk.values()), max(s["started"] for s in chunk.values())
        paths.append({"path": str(path), "sessions": len(chunk), "from": first, "to": last})
    return paths


def command_collect(args):
    today = datetime.strptime(args.today, "%Y-%m-%d").date() if args.today else datetime.now().date()
    start, end = window(today, args.days)
    sessions = collect_window(start, end)
    print(json.dumps({
        "window": {"start": start.isoformat(), "end": end.isoformat()},
        "session_count": len(sessions),
        "prompt_count": sum(len(s["prompts"]) for s in sessions.values()),
        "prompt_chunks": write_chunks(sessions, REPORTS / "data" / today.isoformat()),
        "usage_last_days": collect_usage(args.days),
        "inventory": inventory(sessions),
        "previous_reports": previous_reports(),
        "report_path": str(REPORTS / f"{today.isoformat()}.md"),
    }, indent=1, default=str))


def command_grep(args):
    pattern = re.compile(args.pattern, re.I)
    since = datetime.now().astimezone() - timedelta(days=args.days)
    hits = []
    for path in transcript_files(since):
        for entry in iter_entries(path):
            ts = parse_ts(entry.get("timestamp"))
            if not ts or ts < since:
                continue
            text = human_text(entry)
            if text and pattern.search(text):
                match = pattern.search(text)
                excerpt = text[max(0, match.start() - 120): match.end() + 120].replace("\n", " ")
                hits.append({"at": ts.strftime("%Y-%m-%d %H:%M"), "cwd": entry.get("cwd"), "session": path.stem, "excerpt": excerpt})
    sessions = {hit["session"] for hit in hits}
    print(json.dumps({"pattern": args.pattern, "days": args.days, "hits": len(hits), "sessions": len(sessions), "matches": hits[: args.limit]}, indent=1))


def main():
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    collect = sub.add_parser("collect")
    collect.add_argument("--today", help="YYYY-MM-DD, defaults to the current date")
    collect.add_argument("--days", type=int, default=WINDOW_DAYS)
    grep = sub.add_parser("grep")
    grep.add_argument("pattern")
    grep.add_argument("--days", type=int, default=WINDOW_DAYS)
    grep.add_argument("--limit", type=int, default=20)
    args = parser.parse_args()
    {"collect": command_collect, "grep": command_grep}[args.command](args)


if __name__ == "__main__":
    sys.exit(main())
