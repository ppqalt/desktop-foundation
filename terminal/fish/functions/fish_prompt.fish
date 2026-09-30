function fish_prompt
    set -l last_status $status
    if not set -q __foundation_prompt_ready
        set -g __foundation_prompt_ready 1
        # Autoload event handler once; repainting never runs Git.
        __foundation_git
    end
    set_color $foundation_muted
    printf '%s' (prompt_pwd --dir-length 1)
    if test -n "$__foundation_branch"
        set_color $foundation_accent
        printf ' %s%s' "$__foundation_branch" "$__foundation_dirty"
    end
    if test $last_status -ne 0
        set_color $foundation_error
        printf ' [%s]' $last_status
    end
    set_color $foundation_foreground
    printf ' ❯ '
    set_color normal
end
