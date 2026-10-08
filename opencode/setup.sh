#!/bin/sh

set -eu

config_dir="${XDG_CONFIG_HOME:-$HOME/.config}/opencode"
mkdir -p "$config_dir/session-prs"
cp AGENTS.md "$config_dir/AGENTS.md"
cp opencode.json "$config_dir/opencode.json"
cp session-prs/package.json session-prs/status.js session-prs/registry.js session-prs/commands.js session-prs/tui.tsx "$config_dir/session-prs/"

config="$config_dir/cli.json"
if [ ! -f "$config" ]; then
  printf '{}\n' > "$config"
fi

temporary="$(mktemp "$config_dir/cli.json.XXXXXX")"
trap 'rm -f "$temporary"' EXIT HUP INT TERM
jq '
  .plugins = ((.plugins // []) | if any(.[]; . == "./session-prs" or
    (type == "object" and .package == "./session-prs")) then . else . + ["./session-prs"] end)
' "$config" > "$temporary"
mv "$temporary" "$config"
