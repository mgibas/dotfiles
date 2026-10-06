#!/bin/bash
# Claude Code Status Line with Git support
# Shows: Mode | Model | Effort | Git Branch | Folder | 7d Rate Limit | Tokens

# Read JSON input from stdin (provided by Claude Code)
input=$(cat)

# ANSI color codes
CYAN='\033[01;36m'
GREEN='\033[01;32m'
YELLOW='\033[01;33m'
MAGENTA='\033[01;35m'
BLUE='\033[01;34m'
RED='\033[01;31m'
RESET='\033[00m'
DIM='\033[2m'

# Extract values using jq
MODEL=$(echo "$input" | jq -r '.model.display_name // "Unknown"')
EFFORT=$(echo "$input" | jq -r '.effort.level // empty')
CWD=$(echo "$input" | jq -r '.cwd // .workspace.current_dir // ""')
WEEK_PCT=$(echo "$input" | jq -r '.rate_limits.seven_day.used_percentage // empty')
INPUT_TOKENS=$(echo "$input" | jq -r '.context_window.total_input_tokens // 0')
OUTPUT_TOKENS=$(echo "$input" | jq -r '.context_window.total_output_tokens // 0')
CONTEXT_SIZE=$(echo "$input" | jq -r '.context_window.context_window_size // 200000')
OUTPUT_STYLE=$(echo "$input" | jq -r '.output_style.name // "normal"')
SESSION_ID=$(echo "$input" | jq -r '.session_id // empty')
TRANSCRIPT=$(echo "$input" | jq -r '.transcript_path // empty')

# Calculate context usage
TOTAL_TOKENS=$((INPUT_TOKENS + OUTPUT_TOKENS))
if [ "$CONTEXT_SIZE" -gt 0 ]; then
    CONTEXT_PERCENT=$((TOTAL_TOKENS * 100 / CONTEXT_SIZE))
else
    CONTEXT_PERCENT=0
fi

# Get leaf folder name
if [ -n "$CWD" ]; then
    FOLDER=$(basename "$CWD")
else
    FOLDER=$(basename "$(pwd)")
fi

# Determine mode from output_style
case "$OUTPUT_STYLE" in
    "plan"|"planning") MODE="PLAN" ;;
    "code"|"coding") MODE="CODE" ;;
    "normal"|"default") MODE="AUTO" ;;
    *) MODE=$(echo "$OUTPUT_STYLE" | tr '[:lower:]' '[:upper:]') ;;
esac

# Get git branch (if in a git repo)
GIT_BRANCH=""
if git rev-parse --git-dir &>/dev/null; then
    GIT_BRANCH=$(git symbolic-ref --short HEAD 2>/dev/null || git rev-parse --short HEAD 2>/dev/null)
    # Truncate if too long
    if [ ${#GIT_BRANCH} -gt 30 ]; then
        GIT_BRANCH="${GIT_BRANCH:0:27}..."
    fi
fi

# Build 7-day rate limit progress bar
if [ -n "$WEEK_PCT" ]; then
    WEEK_INT=$(printf '%.0f' "$WEEK_PCT")
    BAR_WIDTH=10
    FILLED=$((WEEK_INT * BAR_WIDTH / 100))
    [ "$FILLED" -gt "$BAR_WIDTH" ] && FILLED=$BAR_WIDTH
    EMPTY=$((BAR_WIDTH - FILLED))
    BAR=$(printf '%0.s█' $(seq 1 $FILLED 2>/dev/null))
    BAR+=$(printf '%0.s░' $(seq 1 $EMPTY 2>/dev/null))
    if [ "$WEEK_INT" -lt 50 ]; then
        LIMIT_COLOR=$GREEN
    elif [ "$WEEK_INT" -lt 80 ]; then
        LIMIT_COLOR=$YELLOW
    else
        LIMIT_COLOR=$RED
    fi
fi

# Color for context percentage
if [ "$CONTEXT_PERCENT" -lt 50 ]; then
    CTX_COLOR=$GREEN
elif [ "$CONTEXT_PERCENT" -lt 80 ]; then
    CTX_COLOR=$YELLOW
else
    CTX_COLOR=$RED
fi

# Build the status line
parts=()
parts+=("${MAGENTA}${MODE}${RESET}")
parts+=("${CYAN}${MODEL}${RESET}")
[ -n "$EFFORT" ] && parts+=("${YELLOW}${EFFORT}${RESET}")
[ -n "$GIT_BRANCH" ] && parts+=("${BLUE}${GIT_BRANCH}${RESET}")
parts+=("${GREEN}${FOLDER}${RESET}")
[ -n "$WEEK_PCT" ] && parts+=("${LIMIT_COLOR}${BAR} ${WEEK_INT}%${RESET} ${DIM}7d${RESET}")
parts+=("${CTX_COLOR}${CONTEXT_PERCENT}%${RESET} ${DIM}(${TOTAL_TOKENS})${RESET}")

# Join with separator
printf '%b' "${parts[0]}"
for ((i=1; i<${#parts[@]}; i++)); do
    printf ' │ %b' "${parts[$i]}"
done

PR_MAX_ROWS=5
PR_CACHE_TTL=60
PR_LOCK_TTL=120
PR_CACHE_DIR="${TMPDIR:-/tmp}/claude-statusline-prs"
PR_CACHE="$PR_CACHE_DIR/$SESSION_ID"
PR_KEYS="$PR_CACHE.keys"
PR_LOCK="$PR_CACHE.lock"

session_prs() {
    grep -F 'pr-link' "$TRANSCRIPT" 2>/dev/null \
        | jq -r 'select(.type == "pr-link" and .prRepository and .prNumber) | "\(.prRepository)\t\(.prNumber)\t\(.prUrl)"' 2>/dev/null \
        | awk '!seen[$0]++'
}

file_age() {
    echo $(( $(date +%s) - $(stat -f %m "$1" 2>/dev/null || echo 0) ))
}

refresh_pr_cache() {
    local prs=$1 tmp="$PR_CACHE.tmp.$$" repo number url me checks
    me=$(gh api user --jq .login 2>/dev/null) || return
    : > "$tmp"
    while IFS=$'\t' read -r repo number url; do
        checks=$(gh pr checks "$number" -R "$repo" --json bucket 2>/dev/null)
        [ -n "$checks" ] || checks='[]'
        gh pr view "$number" -R "$repo" --json number,author,state,isDraft,reviewDecision,title 2>/dev/null \
            | jq -r --arg url "$url" --arg me "$me" --argjson checks "$checks" '
                select(.author.login == $me)
                | ($checks | map(select(.bucket == "fail" or .bucket == "cancel")) | length) as $failed
                | ($checks | map(select(.bucket == "pending")) | length) as $pending
                | [ .number,
                    $url,
                    (if $failed > 0 then "failed:\($failed)"
                     elif $pending > 0 then "pending:\($pending)"
                     elif ($checks | length) > 0 then "passed"
                     else "none" end),
                    (if .state != "OPEN" then .state elif .isDraft then "DRAFT" elif (.reviewDecision // "") == "" then "NONE" else .reviewDecision end),
                    (.title | gsub("\t"; " "))
                  ] | @tsv' >> "$tmp"
    done <<< "$prs"
    mv "$tmp" "$PR_CACHE"
    printf '%s\n' "$prs" > "$PR_KEYS"
}

schedule_pr_refresh() {
    local prs=$1
    [ "$prs" = "$(cat "$PR_KEYS" 2>/dev/null)" ] && [ "$(file_age "$PR_CACHE")" -lt "$PR_CACHE_TTL" ] && return
    [ -d "$PR_LOCK" ] && [ "$(file_age "$PR_LOCK")" -ge "$PR_LOCK_TTL" ] && rmdir "$PR_LOCK" 2>/dev/null
    mkdir -p "$PR_CACHE_DIR"
    mkdir "$PR_LOCK" 2>/dev/null || return
    # Claude Code cancels the status line script when a new update arrives, so gh must not block the render
    ( refresh_pr_cache "$prs"; rmdir "$PR_LOCK" ) </dev/null >/dev/null 2>&1 &
    disown
}

render_pr_rows() {
    local shown=0 total number url checks review title check_label review_label plain width
    total=$(grep -c . "$PR_CACHE" 2>/dev/null)
    while IFS=$'\t' read -r number url checks review title; do
        [ "$shown" -ge "$PR_MAX_ROWS" ] && break
        case "$checks" in
            failed:*)  check_label="${RED}✗ ${checks#failed:} failing" ;;
            pending:*) check_label="${YELLOW}● ${checks#pending:} running" ;;
            passed)    check_label="${GREEN}✓ checks" ;;
            *)         check_label="${DIM}– no checks" ;;
        esac
        case "$review" in
            MERGED)            review_label="${MAGENTA}merged" ;;
            CLOSED)            review_label="${DIM}closed" ;;
            APPROVED)          review_label="${GREEN}approved" ;;
            CHANGES_REQUESTED) review_label="${RED}changes requested" ;;
            REVIEW_REQUIRED)   review_label="${YELLOW}review" ;;
            DRAFT)             review_label="${DIM}draft" ;;
            *)                 review_label="${DIM}no review" ;;
        esac
        plain="#$number │ ${review_label#*m} │ ${check_label#*m} │ "
        width=$(( ${COLUMNS:-120} - ${#plain} - 4 ))
        [ "$width" -lt 10 ] && width=10
        [ ${#title} -gt "$width" ] && title="${title:0:$((width - 3))}..."
        printf '\n%b%s%b' "\e]8;;${url}\a${CYAN}#${number}${RESET}\e]8;;\a │ ${review_label}${RESET} │ ${check_label}${RESET} │ " "$title" "$RESET"
        shown=$((shown + 1))
    done < "$PR_CACHE"
    [ "${total:-0}" -gt "$PR_MAX_ROWS" ] && printf '\n%b' "${DIM}+$((total - PR_MAX_ROWS)) more${RESET}"
}

if [ -n "$SESSION_ID" ] && [ -f "$TRANSCRIPT" ]; then
    PRS=$(session_prs)
    if [ -n "$PRS" ]; then
        schedule_pr_refresh "$PRS"
        [ -s "$PR_CACHE" ] && render_pr_rows
    fi
fi

exit 0
