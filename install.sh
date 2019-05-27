#!/bin/sh

echo "Installing brew..."
/usr/bin/ruby -e "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/master/install)"
brew update

echo "Installing mas..."
brew install mas

echo "Configuring cask..."
brew tap homebrew/cask-versions

#Console apps
brew install git
brew install git-lfs
brew install node@10
brew install yarn
brew install nginx

#NPM Global packages
npm install -g serve
npm install -g markserv
npm install -g @vue/cli

#AppStore apps
mas install 441258766 #Magnet
mas install 1333542190 #1Password
mas install 1090488118 #Gemini
mas install 1254981365 #Contrast — Color Accessibility
mas install 904280696 #Things3
mas install 409201541 #Pages
mas install 409203825 #Numbers
mas install 803453959 #Slack

#Cask apps
brew cask install google-chrome
brew cask install firefox
brew cask install sketch
brew cask install docker
brew cask install parallels13
