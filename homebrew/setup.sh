#!/bin/sh

if test ! $(which brew); then
    echo "Installing homebrew..."
    ruby -e "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/master/install)"
fi

read -r -p "Please login to the AppStore manually and press enter when logged in..."
brew update
brew install mas
brew bundle