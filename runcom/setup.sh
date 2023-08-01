#!/bin/sh

curl -sS https://starship.rs/install.sh | sh -s -- -y

cp .zshrc ~/.zshrc
cp starship.toml ~/.config/starship.toml