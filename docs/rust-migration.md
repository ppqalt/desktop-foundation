# Rust migration inventory

This inventory covers installed behavior and its setup/recovery tools. Tests,
benchmarks, smoke probes and development tooling are excluded. A language's share
of the repository does not describe its startup cost or resident memory use.

Application launch, MIME default ownership and restoration now use the shared
Rust backend. `scripts/application_roles.py` retains installer metadata lookup
and compatibility calls for Python callers; it does not own the MIME transaction.

## Remaining Python implementation

| Area | Source files | Useful migration scope |
| --- | --- | --- |
| Wallpaper/theme transaction | `scripts/theme_runtime.py`, `scripts/theme_pipeline.py` | Bundle staging, validation, publication, component reloads and rollback. Semantic palette generation and caching already run in Rust. This is the strongest next target. |
| Theme output rendering | `theme/adapters/__init__.py`, `theme/adapters/btop.py`, `scripts/render_niri.py`, `scripts/render_terminal.py` | Generate the existing compositor, terminal, shell and application outputs from the same semantic palette in Rust. Preserve the current output formats. |
| Overview backdrop | `scripts/niri_wallpaper.py` | Pillow image validation, scaling and Gaussian blur. Ordinary wallpaper startup is native; this code runs during generation and the legacy no-bundle recovery path. Match the existing image processing before replacing it. |
| Spotify/Spicetify | `scripts/spotify_setup.py`, `scripts/spotify_theme.py` | Pinned client/tool provisioning, patch ownership, upgrades, theme rendering and refresh. The Rust app-mod coordinator currently invokes the Python setup helper. |
| Brave profile setup | `browser/brave/config.py` | Allowlisted export, seeding and restoration of a closed profile. Managed browser theme color updates already use Rust. Preserve the closed-profile checks and separate ownership journal. |
| Deployment and installer | `scripts/deploy.py`, `scripts/install_engine.py`, `scripts/session_units.py`, `scripts/local_ai_launcher.py` | Link ownership, configuration generation, install sequencing and generated service/launcher files. Local AI launch/status behavior is already native. Bootstrap metadata inspection must remain usable before the backend is built. |
| Preference coordination | `scripts/preferences.py` | Command parsing and coordination of native preference recovery with native MIME recovery. The preference journal and GSettings operations already run in Rust. |
| Privileged system setup | `scripts/system-setup`, `scripts/greeter-console-setup`, `scripts/browser-policy-setup`, `scripts/apparmor-setup`, `scripts/boot-setup`, `scripts/boot-optimize`, `scripts/boot_policy.py` | Root-owned helper/PAM/policy deployment, greeter configuration, AppArmor and boot configuration. Consolidate shared file ownership/recovery primitives before moving these. Preserve narrow privileges and original backups. |
| Installation/session diagnostics | `scripts/doctor`, `scripts/preflight.py`, `scripts/session-acceptance` | Read-only package, service, configuration and session checks. Useful for sharing parsers with the backend, but these are explicit commands rather than resident workers. |

The small compatibility entries `scripts/bluetooth-action.py`,
`scripts/bluetooth-power.py`, `scripts/clipboard.py`, `scripts/screenshot.py`,
`scripts/volume.py` and `scripts/niri_notifications.py` enter Rust. The
`scripts/gtk_theme.py` and `scripts/brave_policy.py` bridges also delegate their
behavior to Rust. `scripts/install` enters the installer rather than implementing
a separate installation path. Removing these bridges can simplify interfaces,
but migrating a bridge alone does not replace substantial behavior.

`terminal/kitty/select_all.py` is a supported Kitty kitten that accesses Kitty's
selection state. Keep it in Python: an external Rust command does not provide the
same internal API.

## Remaining shell behavior

| Area | Source files | Recommendation |
| --- | --- | --- |
| Session/service orchestration | `scripts/session-start`, `scripts/session-exit`, `scripts/run-shell`, `scripts/shell-reload`, `scripts/shell-stop`, `scripts/clipboard-start` | Environment import, service control and bounded shell restart handling can join the native session module. `run-shell` ultimately executes Quickshell. |
| Application launch/lifecycle | `scripts/launch`, `scripts/spotify` | Compositor-aware launching and Spotify's first-login/normal-quit patch step can move alongside native application handling. |
| Package/bootstrap setup | `scripts/bootstrap`, `scripts/paru-bootstrap` | Retain a small shell bootstrap for installing the toolchain and reviewing an AUR recipe before the backend exists. Avoid a circular requirement on the binary being installed. |
| Setup/recovery coordination | `scripts/post-install`, `scripts/uninstall`, `scripts/dark-mode` | Consolidate with the native installer/recovery interface when those operations move. |

Most other shell commands are short `exec` launchers for the backend, installer,
theme pipeline or a packaged executable. Examples include `scripts/foundation`,
`scripts/applications`, `scripts/clipboard`, `scripts/volume`, `scripts/monitor`,
`scripts/local-ai`, `scripts/install-app-mods` and `scripts/geode-setup`. They do
not remain as additional resident shell processes after `exec`.

Fish configuration stays in Fish: `terminal/fish/config.fish`,
`terminal/fish/theme.fish`, `terminal/fish/functions/fish_prompt.fish`,
`terminal/fish/functions/__foundation_git.fish` and
`terminal/fish/functions/fast.fish`. Git prompt queries are a possible focused
optimization; replacing the prompt language itself would add complexity.

## JavaScript and Lua

| Source files | Current use | Recommendation |
| --- | --- | --- |
| `scripts/brave_theme_cdp.mjs` | Optional, finite, approval-based DevTools extension/theme lifecycle helper. | Could move to Rust to remove its Node dependency. It is separate from the native managed-policy adapter and is not a startup service. |
| `shell/utils/Search.js` | Small search normalization/ranking library inside QML/Quickshell. | Keep UI-local unless a native search model provides a measurable benefit; it does not start Node. |
| `config/input.lua`, `config/window-appearance.lua`, `theme/window-colors.lua` | Shared settings, palette lookup and generated color data read by renderers/compositor configuration. | Shared data could become TOML/JSON to remove render-time Lua interpretation. |
| `compositor/hyprland/animations.lua`, `compositor/hyprland/appearance.lua`, `compositor/hyprland/bindings.lua`, `compositor/hyprland/hyprland.lua`, `compositor/hyprland/input.lua`, `compositor/hyprland/surfaces.lua`, `profiles/default/monitors.lua` | Native Hyprland configuration and callbacks. | Retain the compositor's supported Lua API; a Rust renderer may produce configuration without replacing the native API. |
| `profiles/default/hardware.lua`, `profiles/default/input.lua` | Profile override placeholders. | No substantive behavior to migrate. |

QML remains responsible for visual surfaces. The Kitty corner shader remains
Slang. The next migration order should be theme transactions/rendering, Spotify
theme ownership, then deployment/recovery coordination; keep bootstrap and
native application configuration small and idiomatic.
