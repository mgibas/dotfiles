#!/bin/sh

source ./defaults.sh
source ./terminal.sh
source ./alias.sh

#sudo console
echo "auth sufficient pam_tid.so" | cat - /etc/pam.d/sudo | sudo tee /etc/pam.d/sudo

killall SystemUIServer
