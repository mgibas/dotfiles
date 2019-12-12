# Add RVM to PATH for scripting. Make sure this is the last PATH variable change.
export PATH="$PATH:$HOME/.rvm/bin"
# History in cache directory:
HISTSIZE=10000
SAVEHIST=10000

## Completitions
autoload -Uz compinit
autoload -U zmv
zmodload zsh/complist
zstyle ':completion:*' matcher-list 'm:{a-zA-Z}={A-Za-z}'
zstyle ':completion:*' menu select
compinit
_comp_options+=(globdots)		# Include hidden files.

fpath=($(brew --prefix)/share/zsh-sie-functions $fpath)


# Use lf to switch directories and bind it to ctrl-o
lfcd () {
    tmp="$(mktemp)"
    lf -last-dir-path="$tmp" "$@"
    if [ -f "$tmp" ]; then
        dir="$(cat "$tmp")"
        rm -f "$tmp"
        [ -d "$dir" ] && [ "$dir" != "$(pwd)" ] && cd "$dir"
    fi
}
bindkey -s '^o' 'lfcd\n'

## Options
setopt  autocd autopushd

## Aliases
alias g="git"
alias rr="rm -rf"

alias l="ls -lahA -G"
alias ll="ls -lA -G"

alias ..="cd .."
alias ...="cd ../.."
alias ....="cd ../../.."

alias zcp='zmv -C'
alias zln='zmv -L'

# Network
alias ip="dig +short myip.opendns.com @resolver1.opendns.com"
alias ipl="ifconfig | grep -Eo 'inet (addr:)?([0-9]*\.){3}[0-9]*' | grep -Eo '([0-9]*\.){3}[0-9]*' | grep -v '127.0.0.1'"

# Miscellaneous
alias week="date +%V"
alias speedtest="wget -O /dev/null http://speed.transip.nl/100mb.bin"

eval "$(starship init zsh)"

# Load zsh-syntax-highlighting; should be last.
source /usr/local/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh
