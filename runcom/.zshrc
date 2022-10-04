# NVM

export NVM_DIR="$HOME/.nvm"
    [ -s "$(brew --prefix)/opt/nvm/nvm.sh" ] && \. "$(brew --prefix)/opt/nvm/nvm.sh"
    [ -s "$(brew --prefix)/opt/nvm/etc/bash_completion.d/nvm" ] && \. "$(brew --prefix)/opt/nvm/etc/bash_completion.d/nvm"

## Aliases
alias g="git"
alias ddc="docker compose"
alias rr="rm -rf"
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
compinit
_comp_options+=(globdots)

eval "$(starship init zsh)"