#!/bin/sh

source ./defaults.sh

#sudo console
echo "auth sufficient pam_tid.so" | cat - /etc/pam.d/sudo | sudo tee /etc/pam.d/sudo

killall SystemUIServer
