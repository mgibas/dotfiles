#!/bin/sh

mkdir ~/Projects
cd ~/Projects

#Create never index file for MacOS Spotlight
touch .metadata_never_index

#Get recent repos from GitHub
read -p "GitHub username: " GITHUB_USERNAME
echo                       
read -s -p "GitHub access token: " GITHUB_TOKEN
echo

curl --user "$GITHUB_USERNAME:$GITHUB_TOKEN" https://api.github.com/users/$GITHUB_USERNAME/events | python -c "import sys, json;import numpy; respo = json.load(sys.stdin); repos = ['https://github.com/' + event['repo']['name'] + '.git' for event in respo]; unique = list(set(repos)); unique.sort(); print '\n'.join(unique)" | xargs -L 1 git clone

cd -
