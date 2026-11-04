# Updating Desktop Foundation

```sh
df-update --dry-run
df-update
```

To select an existing release tag, use `df-update --version 0.3-3`. Version
strings are treated as tag names, not numbers; forms such as `0.33` and
`0.001.1` remain distinct. An exact tag is preferred, then the equivalent with
or without a leading `v`. `--dry-run --version TAG` displays the requested
target without fetching it. Selecting a release currently requires it to be a
descendant of the checkout; older or divergent releases stop without deployment.

Deployment installs `~/.local/bin/df-update`. You can also run
`./scripts/df-update` from the owning checkout. Run it as your desktop user,
without sudo. Keep `~/.local/bin` on your PATH.

The command fetches `main` from the checkout's `origin`, which must be the
Desktop Foundation GitHub repository. It builds both Rust backends using the
existing pinned, locked build scripts in a temporary Git worktree, reuses cached
build dependencies, and validates the compositor configuration before changing
the live checkout or executables. Local settings and extra project files are
included in that isolated build. Diverging local commits are merged there first;
the live checkout advances only after the combined result passes validation.
Local commits are not reset or rebased.

After building, it invokes the existing deployment helper with the installed
compositor and hardware profile. This regenerates configuration and session
units, keeps the selected wallpaper and semantic palette, and retains original
deployment backups. Update mode does not assign application defaults or global
preferences. Clipboard capture is refreshed by deployment when its session is
active; the updater reloads the current compositor configuration, active shell
and active btop sidebar. It does not start inactive desktop components or restart
the compositor, login manager or applications.

System/AUR packages, Spotify/Geode modifications and privileged
boot/login integration are not installed or upgraded. Cargo may download the
Rust dependencies recorded in the repository's lockfiles; builds use the pinned
Rust toolchain from the build scripts. Missing build or deployment dependencies
stop the update; install those separately, then retry. Optional runtime tools
such as `playerctl` are reported without blocking a configuration update.
The full installer remains the route for changing system integration or packages.

## Local settings and errors

Tracked local edits are preserved automatically, including their staged/unstaged
state. A local Git recovery ref under `refs/desktop-foundation/local-update-*`
retains the original snapshot. Publication temporarily autostashes the changes
and restores the already-validated result; your existing stash entries stay intact.
Untracked project files are copied into the isolated build and remain in place.

Conflicting edits, untracked files that would be overwritten, an in-progress Git
operation, or externally replaced deployment links stop the update before deployment.
Changing project files during the build also stops publication, so newer edits
are not silently replaced. The update log records the recovery ref when one is made;
these local backup refs are not included in an ordinary push of `main`.

The updater also retains writable Fish variables, btop preferences, application
state and user overrides under `$XDG_CONFIG_HOME/desktop-foundation`. Changes to
managed repository files need to be kept separately from upstream changes.

Fetch, build and preflight failures leave the live checkout and executables
unchanged. Publication, deployment or reload errors report the failed phase and
retain the deployment journals/backups; resolve the cause and rerun `df-update`,
even if Git is already current. `$XDG_STATE_HOME/desktop-foundation/update.json`
records the previous and intended revisions until the update finishes. The
updater does not automatically reset Git after a partially completed deployment.

Temporary build worktrees are removed on ordinary completion or failure. Forced
termination can leave one under
`$XDG_CACHE_HOME/desktop-foundation/update/source-*`; inspect it with
`git worktree list` before removing it. Build caches remain there for later runs.
`--dry-run` does not fetch, build, deploy or reload anything.

## Output

Normal updates print four short lines: fetching, building/checking, applying, and
the resulting revision. Git, compiler, validation and deployment details go to
`$XDG_STATE_HOME/desktop-foundation/update.log` (default
`~/.local/state/desktop-foundation/update.log`). The file is private to your user
(mode 0600) and replaced at the start of the next real update. Failures identify
the phase and link to this log. `--dry-run` prints one line and creates no log.
