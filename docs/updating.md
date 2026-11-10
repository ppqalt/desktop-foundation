# Updating Desktop Foundation

```sh
df-update --dry-run  # local plan, no network or changes
df-update --check    # read-only live GitHub check
df-update --local --dry-run # preview rebuilding local files
df-update --local    # rebuild local checkout and edits, no GitHub fetch
df-update            # terminal picker: branch, then version
df-update --branch main  # scripted latest-source update, no picker
```

In a terminal, plain `df-update` presents a compact two-stage numbered menu with
pagination and cancellation. Choose **0 — Rebuild from local files** to build
the owning checkout, including staged, unstaged and untracked source files,
without fetching or merging GitHub source. It uses the same isolated builds,
preflight validation, backup and deployment safeguards. Cargo can still download
missing dependencies. `--local` bypasses the picker and `--local --dry-run`
previews this operation. Local mode cannot be combined with branch, version or
GitHub-check options. Otherwise, first choose the branch; then choose its latest
source or a tag reachable in that branch's history. A final summary asks before
building/deploying. The menu uses terminal colors (`NO_COLOR` disables them),
normal line input and no raw terminal mode. `--select` explicitly opens it.

Release entries are Git tags, not downloaded release binaries. Branch/tag metadata
is inspected in a temporary private bare repository, removed on cancellation or
ordinary completion. Cancelling never builds or deploys the desktop. The existing
no-downgrade protection still applies to older releases.

Local mode uses the checkout recorded as the deployment owner in
`$XDG_STATE_HOME/desktop-foundation/manifest.json`. It includes that checkout's
commits and edits, but cannot see new commits that exist only on another local
worktree's branch. Completed development must first be reconciled into the owner;
then `df-update --local --dry-run` shows its new commit before rebuilding.

To choose a GitHub branch for one update:

```sh
df-update --check --branch release/candidate
df-update --branch release/candidate
```

Replace `release/candidate` with an existing branch on the official repository.
`--branch=NAME` also works, including branch names containing `/`. Without this
option, an interactive terminal opens the picker; a noninteractive run defaults
to `main`. The selection is not saved. `--branch NAME --version TAG` selects a
release and verifies it belongs to that branch. A missing branch stops before
building/deployment.

The selected GitHub branch supplies upstream source; it does **not** switch the
installed local branch. Existing local commits and edits are combined with that
source under the normal validation rules. This is an update, not an exact clean
switch or rollback: changes already installed from another branch remain part
of local history. Conflicts stop safely. Do not select unfinished experimental
branches unless you intend to include their changes.

`main` is a branch of the source repository, not a different kind of download.
A source tree is the code at a particular commit, whether selected by a branch
or release tag. `main` usually tracks current development; a tag names a release.

To select an existing release tag, use `df-update --version 0.3-3`. Version
strings are treated as tag names, not numbers; forms such as `0.33` and
`0.001.1` remain distinct. An exact tag is preferred, then the equivalent with
or without a leading `v`. `--dry-run --version TAG` displays the requested
target without fetching it. Selecting a release currently requires it to be a
descendant of the checkout; older or divergent releases stop without deployment.

Deployment installs `~/.local/bin/df-update`. You can also run
`./scripts/df-update` from the owning checkout. Run it as your desktop user,
without sudo. Keep `~/.local/bin` on your PATH.

A normal run also works when the local updater binary is missing: it first builds
an updater using the checkout's pinned build script in a private cache, then that
updater fetches GitHub and performs the ordinary isolated build and validation.
No separate `scripts/build-backend` command is needed. The bootstrap does not
publish intermediate binaries to the desktop. Cargo/Rust, native build dependencies
and `flock` must already be installed; system packages still require the installer.
`--help` works without a binary; `--check` and `--dry-run` report a missing updater
without bootstrapping, downloading or writing anything.

The updater accepts the named branch that owns the installation, including a
Lucky38 customization branch; it does not switch it to `main`, reset local
commits or touch other worktrees. A detached checkout receives a specific
instruction to create a local branch first.

The command fetches the selected branch (default `main`) from the checkout's `origin`, which must be the
Desktop Foundation GitHub repository. If no `origin` is configured, it fetches
directly from `https://github.com/ppqalt/desktop-foundation.git` without silently
adding or rewriting remotes. A different existing origin is rejected. It builds both Rust backends using the
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
active; the updater reloads compositor configuration when its generated content
changed (or cannot be compared), the active shell
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
Changing project files or switching the owning branch during the build also stops
publication, even if the new branch points to the same commit, so newer edits
are not silently replaced. The update log records the recovery ref when one is made;
these local backup refs are not included in an ordinary push of `main`. A separate
`refs/desktop-foundation/update-before-*` ref retains the previous committed
revision for every validated publication. Pending `update.json` also records the
owning branch, upstream revision and recovery ref.

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
`--dry-run` does not access the network, fetch, build, deploy or reload anything.
`--check` queries GitHub with `git ls-remote` and reports the advertised revision;
it does not fetch objects, create logs, build, deploy or alter the checkout.
When the GitHub revision is already present locally, it distinguishes local
commits from new upstream work rather than treating any different hash as an
available upgrade.

## Output

Normal updates print four short lines: fetching, building/checking, applying, and
the resulting branch and revision. Git, compiler, validation and deployment details go to
`$XDG_STATE_HOME/desktop-foundation/update.log` (default
`~/.local/state/desktop-foundation/update.log`). The file is private to your user
(mode 0600) and replaced at the start of the next real update. Failures identify
the phase and link to this log. `--dry-run` shows the owning checkout, source and plan and creates no log.
`--check` reports live GitHub metadata separately. `--help` lists both modes.
