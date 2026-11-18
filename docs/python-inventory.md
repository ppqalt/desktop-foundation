# First-party Python inventory

Python remains where it has functional callers. This is a caller map, not a
requirement to port working code or delete regression coverage.

## Installation and application integration

| Entry point | Python implementation / caller |
| --- | --- |
| `scripts/apparmor-setup`, `boot-optimize`, `boot-setup`, `browser-policy-setup`, `greeter-console-setup`, `plymouth-setup` | Extensionless Python policy writers; boot helpers use `boot_policy.py` |
| `scripts/boot-integrate` | `scripts/boot_integration.py`; protected standalone boot planner/writer |
| `scripts/performance-profile` | `scripts/performance_profile.py`; reversible privileged profile writer |
| `scripts/brave-config` | `browser/brave/config.py`; browser provisioning, not profile resets |
| `scripts/spotify-setup` | `scripts/spotify_setup.py`; native `check` route remains separate |
| `scripts/install` | Embedded bounded ownership/selection checks before native/bootstrap operations |
| `terminal/kitty/kitty.conf` | Kitty's `terminal/kitty/select_all.py` kitten |
| Native monitor preparation | Kitty's own `+runpy` shader compiler interface |

These still need Python; combining runtime wrappers does not remove that package
dependency. `scripts/brave_policy.py` is also imported by the retained Python
theme compatibility route.

## Imported compatibility modules

The active desktop uses Rust for normal deployment/theme/session operations.
Existing Python tests and older adapters still import these modules:

- Deployment/preferences: `scripts/application_roles.py`, `deploy.py`,
  `preferences.py`, `session_units.py`, `local_ai_launcher.py`.
- Theme/rendering: `scripts/gtk_theme.py`, `render_niri.py`, `render_terminal.py`,
  `spotify_theme.py`, `theme_pipeline.py`, `theme_runtime.py`, and
  `theme/adapters/` (`__init__.py`, `btop.py`).
- Wallpaper/session helpers: `scripts/niri_wallpaper.py`, `niri_notifications.py`.
- Clipboard: `scripts/clipboard.py`; also usable as the explicit isolated QML
  fixture backend, while the normal worker is Rust.
- Executable compatibility bridges: `scripts/bluetooth-action.py` and
  `scripts/screenshot.py`; their forwarding contracts are regression tested.

Do not remove an import-compatible module until its callers and coverage have a
replacement. The unused `bluetooth-power.py` forwarding shim was removed; its
only remaining mentions are inside frozen Git-baseline benchmarks.

## Development-only tools

Located under `tools/` rather than mixed with installed commands:

- `tools/benchmarks/`: Bluetooth action/power, screenshot, theme-generation and
  clipboard quality comparisons. They extract historical sources through
  `git show`; those historical filenames are not current runtime dependencies.
- `tools/live/compositor.py`: explicit focus/window acceptance check.
- `tools/fixtures/gallery-backend`: inert sample data shared by the gallery and
  motion tests.

The obsolete Hyprland-only clipboard and launcher smoke scripts were removed.
Current clipboard/launcher mouse, focus, lifecycle and keyboard tests remain in
`tests/`; no CI checks or regression tests were disabled for this cleanup.

## Tests and discovery

Python tests and fixtures live in `tests/`. CI compiles `scripts/` and `tools/`,
runs Python discovery, native Rust checks, and the required isolated lock UI job.
`./scripts/dev test-python` runs the Python suite locally after building debug
binaries. `./scripts/dev test` runs native and shared offscreen QML tests.

Find actual files rather than maintaining another duplicate filename list:

```sh
rg --files scripts tools tests theme browser terminal -g '*.py'
rg -l '^#!.*python' scripts tools
```

Inspect imports, subprocess calls and tests before removing a file. Package
manifests still include `python` and `python-pillow`; this cleanup does not claim
a Python-free installer or desktop toolchain.
