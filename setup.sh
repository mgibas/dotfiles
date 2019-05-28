#!/bin/sh

# Ask for the administrator password upfront
sudo -v

# Keep-alive: update existing `sudo` time stamp until `.macos` has finished
while true; do sudo -n true; sleep 60; kill -0 "$$" || exit; done 2>/dev/null &

source ./install.sh

cd ./runcom
source ./runcom/setup.sh
cd -

cd ./git
source ./git/setup.sh
cd -

cd ./macos
source ./macos/setup.sh 
cd -

cd ./vim
source ./vim/setup.sh
cd -

cd ./nginx
source ./nginx/setup.sh
cd -

cd ./sketchapp
source ./sketchapp/setup.sh
cd -

cd ./dir
source ./dir/setup.sh 
cd -
