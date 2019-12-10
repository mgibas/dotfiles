# Add RVM to PATH for scripting. Make sure this is the last PATH variable change.
export PATH="$PATH:$HOME/.rvm/bin"

## Completitions
autoload -Uz compinit && compinit
fpath=($(brew --prefix)/share/zsh-sie-functions $fpath)

## Options
setopt  autocd autopushd

# case-insensitive (uppercase from lowercase) completion
zstyle ':completion:*' matcher-list 'm:{a-zA-Z}={A-Za-z}'

## Aliases
alias g="git"
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
alias speedtest="wget -O /dev/null http://speed.transip.nl/100mb.bin"

eval "$(starship init zsh)"
