#!/bin/sh

# Ask for the administrator password upfront
sudo -v

# Keep-alive: update existing `sudo` time stamp until `.macos` has finished
while true; do sudo -n true; sleep 60; kill -0 "$$" || exit; done 2>/dev/null &

cd ./homebrew
source ./setup.sh
cd -

cd ./node
source ./setup.sh
cd -

cd ./runcom
source ./setup-macos.sh
cd -

cd ./git
source ./setup.sh
cd -

cd ./macos
source ./setup.sh 
cd -

cd ./vim
source ./setup.sh
cd -

cd ./dir
source ./setup.sh 
cd -
