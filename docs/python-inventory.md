# First-party Python inventory

Snapshot: after the native startup and failure diagnostics and QML test-runner migrations. Paths below are repository files; build artifacts and third-party code are excluded. Update this inventory after each migration. Python imports and legacy test coverage must be checked before deleting compatibility modules.

## Runtime entry points without a `.py` suffix

These are Python executables used for privileged setup or policy. The shell wrappers listed below also dispatch to Python.

```
scripts/apparmor-setup
scripts/boot-optimize
scripts/boot-setup
scripts/browser-policy-setup
scripts/greeter-console-setup
```

## Executable tools and entry points

Python files with a Python shebang or a `__main__` entry point. Some are legacy compatibility entry points whose active replacements are already Rust; inspect callers and tests before removal.

```
browser/brave/config.py
scripts/application_roles.py
scripts/bluetooth-action.py
scripts/bluetooth-actions-bench.py
scripts/bluetooth-bench.py
scripts/bluetooth-power.py
scripts/clipboard-smoke.py
scripts/clipboard.py
scripts/deploy.py
scripts/launcher-smoke.py
scripts/niri_notifications.py
scripts/niri_wallpaper.py
scripts/preferences.py
scripts/quality_bench.py
scripts/render_terminal.py
scripts/screenshot-bench.py
scripts/screenshot.py
scripts/smoke.py
scripts/spotify_setup.py
scripts/spotify_theme.py
scripts/theme-generation-bench.py
scripts/theme_pipeline.py
```

## Libraries and compatibility modules

```
scripts/boot_policy.py
scripts/brave_policy.py
scripts/gtk_theme.py
scripts/local_ai_launcher.py
scripts/render_niri.py
scripts/session_units.py
scripts/theme_runtime.py
terminal/kitty/select_all.py
theme/__init__.py
theme/adapters/__init__.py
theme/adapters/btop.py
```

## Tests and fixtures

```
tests/live_nothing_surface.py
tests/live_session_surface.py
tests/preference_fixture.py
tests/test_application_roles.py
tests/test_backdrop.py
tests/test_backend.py
tests/test_backend_bluetooth.py
tests/test_boot_optimization.py
tests/test_boot_policy.py
tests/test_brave_config.py
tests/test_clipboard.py
tests/test_clipboard_qml_focus.py
tests/test_clipboard_qml_mouse.py
tests/test_deploy.py
tests/test_gtk_theme.py
tests/test_install.py
tests/test_menu_lifecycle.py
tests/test_native_bluetooth_actions.py
tests/test_native_preferences.py
tests/test_native_screenshot.py
tests/test_native_session.py
tests/test_native_theme.py
tests/test_niri_adapter.py
tests/test_nothing_qml_qol.py
tests/test_preferences.py
tests/test_session_startup.py
tests/test_spotify_theme.py
tests/test_spotify_upgrade.py
tests/test_surface_keyboard.py
tests/test_theme_pipeline.py
tests/test_theme_runtime.py
tests/test_update.py
```

## Runtime and tooling invocations

- `scripts/apparmor-setup`, `scripts/boot-optimize`, `scripts/boot-setup`, `scripts/browser-policy-setup` and `scripts/greeter-console-setup` invoke Python through their shebangs.
- `scripts/brave-config` and `scripts/spotify-setup` invoke Python explicitly; Spotify's native `check` path is separate.
- `terminal/kitty/kitty.conf` invokes the Python Kitty kitten `select_all.py`.
- The monitor shader preparation in `native/foundation/src/monitor.rs` invokes
  Kitty's `+runpy` interface for Kitty's own shader pipeline compiler.
- `shell/services/ClipboardHistory.qml` supports an optional Python fixture override; its normal backend is Rust.
- `packages/core.toml` still includes `python` and `python-pillow`; `.github/workflows/check.yml` installs Python tooling and compiles/runs the remaining Python tests.
- Some Python benchmark tools load historical Python baselines. Migrate their measurements and baseline strategy before removing their runtime requirement.

Find current Python source with `rg --files -g '*.py'`; find extensionless entry points with `rg -l '^#!.*python' scripts`. Search invocation and import sites, not just filenames, when updating this list.
