# Keep noninteractive shells free of presentation and interactive helpers.
if not status is-interactive
    return
end
source (path dirname (status filename))/theme.fish
set -g fish_greeting
abbr -a -- c clear
abbr -a -- .. 'cd ..'
abbr -a -- ... 'cd ../..'
abbr -a -- gs 'git status --short --branch'
abbr -a -- gd 'git diff'
abbr -a -- gl 'git log --oneline -12'
# Native Fish autosuggestions, history pager and key bindings are retained.
