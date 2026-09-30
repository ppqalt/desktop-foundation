function __foundation_git --on-event fish_postexec --on-variable PWD
    set -g __foundation_branch
    set -g __foundation_dirty
    command -q git; or return
    # Tracked changes only: no recursive untracked scan, submodule scan or ahead count.
    set -l snapshot (command git --no-optional-locks status --porcelain=v2 --branch --untracked-files=no --ignore-submodules=dirty --no-ahead-behind 2>/dev/null)
    for line in $snapshot
        if string match -q '# branch.head *' -- $line
            set -g __foundation_branch (string sub -s 15 -- $line)
        else if string match -qr '^[12u] ' -- $line
            set -g __foundation_dirty '*'
        end
    end
    if test "$__foundation_branch" = '(detached)'
        set -g __foundation_branch detached
    end
    set -g __foundation_branch (string shorten -m 32 -- $__foundation_branch)
end
