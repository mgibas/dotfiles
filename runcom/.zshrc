## Aliases
alias g="git"
alias gs="git status"
alias gd="git diff"
alias gco="git checkout"
alias ddc="docker compose"
alias rr="rm -rf -i"
alias l="ls -lahA -G"
alias ll="ls -lA -G"
alias ..="cd .."
alias ...="cd ../.."
alias ....="cd ../../.."

# Network
alias ip="dig +short myip.opendns.com @resolver1.opendns.com"
alias ipl="ifconfig | grep -Eo 'inet (addr:)?([0-9]*\.){3}[0-9]*' | grep -Eo '([0-9]*\.){3}[0-9]*' | grep -v '127.0.0.1'"

# Miscellaneous
alias week="date +%V"

# Case insensitive completion
autoload -Uz compinit
autoload -U zmv
zmodload zsh/complist
zstyle ':completion:*' matcher-list 'm:{a-zA-Z}={A-Za-z}'
zstyle ':completion:*' menu select
# Only regenerate compinit dump once per day
() {
  if (( $# )); then
    compinit
  else
    compinit -C
  fi
} ~/.zcompdump(N.mh+24)
_comp_options+=(globdots)

# History
HISTFILE=~/.zsh_history
HISTSIZE=10000
SAVEHIST=10000
setopt SHARE_HISTORY
setopt HIST_IGNORE_DUPS
setopt HIST_IGNORE_SPACE

# Directory navigation
setopt AUTO_CD
setopt AUTO_PUSHD
setopt PUSHD_SILENT

# Keybindings
bindkey '^[[A' history-search-backward
bindkey '^[[B' history-search-forward

eval "$(starship init zsh)"
export PATH="$HOME/.local/bin:$PATH"

# Lazy load GITHUB_TOKEN on first command
_lazy_load_github_token() {
  if [[ -z "$GITHUB_TOKEN" ]]; then
    export GITHUB_TOKEN="$(gh auth token 2>/dev/null)"
  fi
  add-zsh-hook -d preexec _lazy_load_github_token
}
autoload -Uz add-zsh-hook
add-zsh-hook preexec _lazy_load_github_token

# pnpm
export PNPM_HOME="$HOME/Library/pnpm"
case ":$PATH:" in
  *":$PNPM_HOME:"*) ;;
  *) export PATH="$PNPM_HOME:$PATH" ;;
esac
# pnpm end
