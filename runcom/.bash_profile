export PATH=~/Library/Python/3.6/bin/:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin

for f in $(brew --prefix)/etc/bash_completion.d/*; do source $f; done

source ~/.prompt_profile
