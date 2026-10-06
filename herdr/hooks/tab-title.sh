#!/bin/bash
[[ "$HERDR_ENV" == 1 && -n "$HERDR_PANE_ID" ]] || exit 0

HERDR="${HERDR_BIN_PATH:-herdr}"
STATE_DIR="$HOME/.local/state/herdr-tab-title"
MAX_LENGTH=40

input="$(cat)"
pane="$("$HERDR" pane get "$HERDR_PANE_ID" 2>/dev/null)" || exit 0
tab_id="$(jq -r '.result.pane.tab_id // empty' <<<"$pane")"
[[ -n "$tab_id" ]] || exit 0

session_id="$(jq -r '.session_id // empty' <<<"$input")"
title=""
if [[ -n "$session_id" && -f "$HOME/.codex/session_index.jsonl" ]]; then
  title="$(jq -r --arg id "$session_id" 'select(.id == $id) | .thread_name // empty' "$HOME/.codex/session_index.jsonl" | tail -1)"
fi
[[ -n "$title" ]] || title="$(jq -r '.result.pane.terminal_title_stripped // empty' <<<"$pane")"
[[ -n "$title" && ! "$title" =~ ^([Cc]laude( Code)?|[Cc]odex)$ ]] || exit 0
(( ${#title} > MAX_LENGTH )) && title="${title:0:$((MAX_LENGTH - 1))}…"

tab="$("$HERDR" tab get "$tab_id" 2>/dev/null)" || exit 0
label="$(jq -r '.result.tab.label // empty' <<<"$tab")"
pane_count="$(jq -r '.result.tab.pane_count // 0' <<<"$tab")"
(( pane_count == 1 )) || exit 0

marker="$STATE_DIR/${tab_id//:/_}"
[[ "$label" =~ ^[0-9]+$ || ( -f "$marker" && "$label" == "$(cat "$marker")" ) ]] || exit 0
[[ "$label" != "$title" ]] || exit 0

mkdir -p "$STATE_DIR"
"$HERDR" tab rename "$tab_id" "$title" >/dev/null 2>&1 && printf '%s' "$title" >"$marker"
exit 0
