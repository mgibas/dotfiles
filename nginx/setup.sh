#!/bin/sh

cp nginx.conf /usr/local/etc/nginx/nginx.conf
cp nginx.server.template /usr/local/etc/nginx/servers/template.localhost
cp /usr/local/opt/nginx/*.plist /Library/LaunchDaemons
launchctl load -w /Library/LaunchDaemons/homebrew.mxcl.nginx.plist
