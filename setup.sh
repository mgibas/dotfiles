#!/bin/sh

# Ask for the administrator password upfront
sudo -v

# Keep-alive: update existing `sudo` time stamp until `.macos` has finished
while true; do sudo -n true; sleep 60; kill -0 "$$" || exit; done 2>/dev/null &

source ./install.sh

cd ./runcom
source ./setup.sh
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

cd ./nginx
source ./setup.sh
cd -

cd ./sketchapp
source ./setup.sh
cd -

cd ./dir
source ./dir/setup.sh 
cd -
