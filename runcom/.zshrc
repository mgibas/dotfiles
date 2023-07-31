## Aliases
alias g="git"
alias ddc="docker compose"
alias rr="rm -rf"
alias l="ls -lahA -G"
alias ll="ls -lA -G"
alias ..="cd .."
alias ...="cd ../.."
alias ....="cd ../../.."

# Case insensitive completion
autoload -Uz compinit
autoload -U zmv
zmodload zsh/complist
zstyle ':completion:*' matcher-list 'm:{a-zA-Z}={A-Za-z}'
zstyle ':completion:*' menu select
compinit
_comp_options+=(globdots)