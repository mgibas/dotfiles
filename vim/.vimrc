" Specify a directory for plugins
" - For Neovim: ~/.local/share/nvim/plugged
" - Avoid using standard Vim directory names like 'plugin'
call plug#begin('~/.vim/plugged')

Plug 'vim-scripts/matchit.zip'
Plug 'scrooloose/nerdtree'
Plug 'scrooloose/nerdcommenter'
Plug 'pangloss/vim-javascript'
Plug 'quramy/vim-js-pretty-template'
Plug 'chrisbra/Colorizer'
Plug 'alvan/vim-closetag'
Plug 'altercation/vim-colors-solarized'
Plug 'w0rp/ale'
Plug 'kien/ctrlp.vim'
Plug 'mattn/emmet-vim'
Plug 'danro/rename.vim'
Plug 'posva/vim-vue'
Plug 'othree/xml.vim'
Plug 'vim-scripts/loremipsum'

call plug#end() 

"Key bindings
map <C-n> :NERDTreeToggle<CR>
noremap <silent><Leader><right> :tabnext<CR>
noremap <silent><Leader><left> :tabprevious<CR>

"Turning off arrow keys
noremap <Up> <NOP>
noremap <Down> <NOP>
noremap <Left> <NOP>
noremap <Right> <NOP>

syntax on
set number
set ruler
set noswapfile
set nowrap

"default indent settings
set shiftwidth=2
set softtabstop=2
set expandtab
set autoindent

"Theme setup
set background=dark
let g:solarized_termcolors=256
let g:solarized_termtrans=1
silent! colorscheme solarized

"Configuring Colorized
:let g:colorizer_auto_filetype='css,scss,html,vue'

"Configure Closetag
let g:closetag_filenames = '*.html,*.xhtml,*.phtml, *.vue'

"Spell checking
set spell

"Configure ALE
let g:ale_linter_aliases = {'vue': ['css', 'javascript']}
let g:ale_linters = {'vue': ['stylelint', 'eslint']}
let g:ale_fixers = {
\   'javascript': ['eslint'],
\   'vue': ['eslint', 'stylelint']
\}

"Functions
" Simple re-format for minified Javascript
command! UnMinify call UnMinify()
function! UnMinify()
    %s/{\ze[^\r\n]/{\r/g
    %s/){/) {/g
    %s/};\?\ze[^\r\n]/\0\r/g
    %s/;\ze[^\r\n]/;\r/g
    %s/[^\s]\zs[=&|]\+\ze[^\s]/ \0 /g
    normal ggVG=
endfunction
