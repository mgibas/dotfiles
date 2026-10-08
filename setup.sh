#!/bin/sh

cd "$(dirname "$0")" || exit 1

# Ask for the administrator password upfront
sudo -v

# Keep-alive: update existing `sudo` time stamp until `.macos` has finished
while true; do sudo -n true; sleep 60; kill -0 "$$" || exit; done 2>/dev/null &

(cd homebrew && . ./setup.sh) || echo "setup failed: homebrew" >&2
eval "$(/opt/homebrew/bin/brew shellenv)"

for step in node runcom git claude codex opencode herdr ghostty macos vim; do
  (cd "$step" && . ./setup.sh) || echo "setup failed: $step" >&2
done
