# Steam / Millennium Material integration: investigation only

Current machine: Steam package 1.0.0.87-3; no Millennium package/loader/theme
installation detected. Steam resolves to ~/.local/share/Steam through
~/.steam/steam. No Steam files, userdata, libraries, launch options, account data,
cloud saves or client branch settings were changed in this pass.

The selected theme is Material, ID ipYjqODds05KMcvh7QJn, upstream
https://github.com/kuska1/Material-Theme (catalog version 1.5.16 when inspected).
Its matugentemplate.css expects `--md-sys-color-<role>` RGB values, not hex values,
plus `--theme-color: "Matugen"`. Its Color=Matugen condition loads the supplied
CSS/JS. A future renderer should emit this existing variable contract from our
semantic palette: surface/background=background, surface containers=elevated,
on-surface=foreground, on-surface-variant=muted, primary=accent,
primary-container=selected, outlines=border/selectionBorder, errors=error.
Do not feed raw Material surfaces back into graphite.

Material's own js/main/colors/matugen.js fetches its existing matugen.css every
1500ms and injects it into Steam windows. Thus once Millennium/Material is loaded,
color refresh is LIVE through upstream's mechanism, but includes upstream polling
cost. Desktop-foundation should add no second watcher or color-injection framework.
Its legacy/new fallback URLs already cover older/newer Millennium layouts.

## Exact next implementation

1. Use the officially supported Arch AUR `millennium` package, with Steam from
   pacman/multilib. At investigation, PKGBUILD was 3.5.0-1 pinned to upstream commit
   765aa8802f8a4d942ad8ac8323a9e0f233a50fa8. It builds C++/Rust/frontend code and
   needs cmake/ninja/bun and 32-bit development dependencies. Inspect/package the
   recipe unprivileged; install through the existing AUR bootstrap/paru stage.
   System installation requires the user's normal sudo authentication.
2. Journal only Millennium loader/theme-owned paths. Verify the installed
   version/layout before touching Steam. Do not run a floating installer blindly:
   inspected generic install.sh installs system libraries via sudo, replaces
   libXtst loader symlinks and deletes Steam's package/beta selector to force stable.
   That branch change is unrelated to theming and must not happen.
3. Provision a checksum-pinned Material archive in Millennium's supported theme
   directory, preserving existing unrelated themes and plugins. Official Linux
   documentation lists steamui/skins; Material's newer template lists
   millennium/themes for >=3.0. Validate actual package/runtime path before choosing.
4. Select only active theme=Material, appearance=Dark, Color=Matugen through
   supported Millennium configuration/CLI/IPC. Public docs currently describe a
   native settings install flow; inspect the installed package's API/schema
   before implementing deterministic selection. Never rewrite guessed config.
5. First loader activation needs a normal Steam restart; do not force-kill Steam
   or an active game. Package bootstrap and first launch may require a deferred
   post-install stage. Subsequent theme colors should not require restart.
6. Stage the semantic CSS in runtime bundles and publish into Material's existing
   matugen.css contract with original backup, ownership checks and rollback.
   Optional absent Steam/Millennium should not block other wallpaper targets.

This is materially more involved than adding a color renderer: package build,
privileged loader installation, first-launch lifecycle and version-dependent
configuration need their own validated transaction. No partial Steam adapter,
unverified provisioning script or package dependency has been installed here.
The user's permission to run an installer removes an approval obstacle, but does
not remove these implementation/validation requirements.

Primary sources:
- https://docs.steambrew.app/users/getting-started/installation
- https://docs.steambrew.app/users/getting-started/structure
- https://docs.steambrew.app/users/guides/installing-addons
- https://aur.archlinux.org/cgit/aur.git/plain/PKGBUILD?h=millennium
- https://steambrew.app/install.sh
- https://github.com/kuska1/Material-Theme/blob/main/matugentemplate.css
- https://github.com/kuska1/Material-Theme/blob/main/js/main/colors/matugen.js
