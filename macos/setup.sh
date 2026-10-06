#!/bin/sh

source ./defaults.sh

if ! grep -qs '^auth.*pam_tid.so' /etc/pam.d/sudo_local; then
  sed 's/^#auth/auth/' /etc/pam.d/sudo_local.template | sudo tee /etc/pam.d/sudo_local >/dev/null
fi

killall Dock
killall SystemUIServer
