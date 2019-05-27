# Ask for the administrator password upfront
sudo -v

# Keep-alive: update existing `sudo` time stamp until `.macos` has finished
while true; do sudo -n true; sleep 60; kill -0 "$$" || exit; done 2>/dev/null &

# Github account
read -p "GitHub username: " GITHUB_USERNAME
echo                       
read -s -p "GitHub access token: " GITHUB_TOKEN
echo

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

source ./settings.sh 

#Bash profile
cp .prompt_profile ~/.prompt_profile

echo "export PATH=~/Library/Python/3.6/bin/:$PATH" >> ~/.bash_profile
echo 'for f in $(brew --prefix)/etc/bash_completion.d/*; do source $f; done' >> ~/.bash_profile
echo 'source ~/.prompt_profile' >> ~/.bash_profile

#vim config
cp .vimrc ~/.vimrc
curl -fLo ~/.vim/autoload/plug.vim --create-dirs https://raw.githubusercontent.com/junegunn/vim-plug/master/plug.vim
vim +PlugInstall +qall

# nginx
cp nginx.conf /usr/local/etc/nginx/nginx.conf
cp nginx.server.template /usr/local/etc/nginx/servers/template.localhost
cp /usr/local/opt/nginx/*.plist /Library/LaunchDaemons
launchctl load -w /Library/LaunchDaemons/homebrew.mxcl.nginx.plist

#git config
cp .gitconfig ~/.gitconfig
sed -i '.bak' 's/GITHUB_TOKEN/$GITHUB_TOKEN/' ~/.gitconfig

#Projects setup
mkdir ~/Projects
cd ~/Projects
touch .metadata_never_index
curl --user "$GITHUB_USERNAME:$GITHUB_TOKEN" https://api.github.com/users/$GITHUB_USERNAME/events |     python -c "import sys, json;import numpy; respo = json.load(sys.stdin); repos = ['https://github.com/' + event['repo']['name'] + '.git' for event in respo]; unique = list(set(repos)); unique.sort(); print '\n'.join(unique)" | xargs -L 1 git clone

# Sketch plugins
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
