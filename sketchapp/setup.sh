#!/bin/sh

curl -fLo ~/Downloads/craft.zip --create-dirs https://craft-assets.invisionapp.com/CraftManager/production/CraftManager.zip
unzip ~/Downloads/craft.zip -d /Applications/
rm ~/Downloads/craft.zip
open -a CraftManager

curl -fLo ~/Downloads/plugins/symbol-organizer.zip --create-dirs https://github.com/sonburn/symbol-organizer/archive/14.2.zip
curl -fLo ~/Downloads/plugins/rename-it.zip --create-dirs https://github.com/rodi01/RenameIt/releases/download/v3.9.6-2/Rename-It.sketchplugin.zip
unzip ~/Downloads/plugins/symbol-organizer.zip -d ~/Downloads/plugins
unzip ~/Downloads/plugins/rename-it.zip -d ~/Downloads/plugins

mv ~/Downloads/plugins/Rename-It.sketchplugin ~/Library/Application\ Support/com.bohemiancoding.sketch3/Plugins/Rename-It.sketchplugin
mv ~/Downloads/plugins/symbol-organizer-14.2/Symbol\ Organizer.sketchplugin ~/Library/Application\ Support/com.bohemiancoding.sketch3/Plugins/Symbol\ Organizer.sketchplugin

rm -rf ~/Downloads/plugins/
