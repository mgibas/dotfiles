call plug#begin('~/.vim/plugged')

Plug 'vim-scripts/matchit.zip'
Plug 'scrooloose/nerdtree'
Plug 'scrooloose/nerdcommenter'
Plug 'pangloss/vim-javascript'
Plug 'quramy/vim-js-pretty-template'
Plug 'alvan/vim-closetag'
Plug 'w0rp/ale'
Plug 'kien/ctrlp.vim'
Plug 'mattn/emmet-vim'
Plug 'danro/rename.vim'
Plug 'posva/vim-vue'
Plug 'othree/xml.vim'
Plug 'sonph/onehalf', { 'rtp': 'vim' }

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

"Turning of yanking after paste
xnoremap p pgvy

syntax on
set number
set ruler
set noswapfile
set nowrap

"default indent settings
set shiftwidth=2
set softtabstop=2
set tabstop=2
set expandtab
set autoindent

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

"Theme setup

if exists('+termguicolors')
    let &t_8f = "\<Esc>[38;2;%lu;%lu;%lum"
    let &t_8b = "\<Esc>[48;2;%lu;%lu;%lum"
    set termguicolors
endif

syntax on
set t_Co=256
set cursorline
colorscheme onehalfdark
let g:airline_theme='onehalfdark'