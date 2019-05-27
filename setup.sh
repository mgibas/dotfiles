#!/bin/sh

# Ask for the administrator password upfront
sudo -v

# Keep-alive: update existing `sudo` time stamp until `.macos` has finished
while true; do sudo -n true; sleep 60; kill -0 "$$" || exit; done 2>/dev/null &

#Bash profile
cp .bash_profile ~/.bash_profile
cp .prompt_profile ~/.prompt_profile

source ./install.sh
source ./git/setup.sh
source ./macos/setup.sh 
source ./vim/setup.sh
source ./nginx/setup.sh
source ./sketchapp/setup.sh
source ./dir/setup.sh 
