#!/bin/sh

# Menu bar: show battery percentage
defaults write com.apple.menuextra.battery ShowPercent YES

# Automatically hide and show the Dock
defaults write com.apple.dock autohide -bool true

# Require password immediately after sleep or screen saver begins
defaults write com.apple.screensaver askForPassword -int 1
defaults write com.apple.screensaver askForPasswordDelay -int 0

# MacOS dark theme
defaults write -g AppleInterfaceStyle Dark
