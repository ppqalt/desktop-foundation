# Personal configuration

Keep personal overrides in `~/.config/desktop-foundation/` (or
`$XDG_CONFIG_HOME/desktop-foundation/`). Only specify the values you want to
change. Updates retain these files; deleting a value restores the shared default.

`desktop.toml`:

```toml
[media]
home_player = "spotify"

[screen_corners]
enabled = true
radius = 28
cover_fullscreen = true
```

Home prefers the named controllable MPRIS session, even when paused or stopped.
If it is unavailable, Home selects the most recently used player other than
End's primary player. An empty `home_player` restores secondary-only selection.
Instance-suffixed Spotify sessions are supported. End and Insert/Delete keep
their existing primary-player behavior. No new media service or polling is used.

`window-appearance.toml`:

```toml
rounding = 14
[overview]
corner_radius = 24
feather_enabled = true
feather_width = 4
[shadow]
spread = 12
range = 32
activeOpacity = 1.0
inactiveOpacity = 1.0
[opacity]
active = 0.94
```

Feathering defaults to a subtle **4 logical pixels**. It affects confirmed fullscreen
workspace previews only. Corner radius and feather width accept 0–128 logical
pixels. Physical screen-corner radius accepts integers 0–128; zero disables the
cutouts. Shadow spread accepts 0–128. These are desktop adaptations, not new
Material tokens.

`feather_enabled=false` retains crisp rounding without feathering.
`screen_corners.cover_fullscreen=false` keeps corner masks below native fullscreen
apps, favoring direct scanout; the default covers fullscreen too.

Appearance precedence, lowest first:

1. Repository `config/window-appearance.toml`.
2. `profiles/NAME/window-appearance.toml`.
3. User `window-appearance.d/*.toml`, sorted by filename.
4. User `window-appearance.toml`.

Desktop settings use the same repository, sorted user fragments and final user
file order, without a hardware-profile layer. Invalid TOML, unknown override
keys, wrong types and out-of-range new settings fail validation. Each input is
limited to 64 KiB. No configuration file is executed or rewritten.

Inspect effective settings and their source paths:

```sh
./scripts/foundation config desktop
./scripts/foundation config appearance --profile lucky38
./scripts/foundation config input --profile lucky38
```

Apply settings without rebuilding or restarting Niri:

```sh
./scripts/foundation config reload
```

Reload retains the active hardware profile, wallpaper and palette. It stages and
validates a new immutable theme revision before publication, uses the existing
rollback path if publication/reload fails, and refreshes shell settings
asynchronously. It does not start a configuration daemon. Shell-only preferences
also load at shell startup; failed reads retain the current shell values and
report the error in its log. Runtime acceptance still requires the relevant
hardware/session.

The managed Niri package capability gates custom overview settings independently
of whether Ctrl+Scroll is enabled. Stock recovery omits unsupported overview
nodes while retaining TOML preferences for later reactivation.

Other existing settings remain in their existing TOML files: `input.toml`,
`monitor.toml` (shared sidebar dimensions and motion), `recording.toml`,
`screenshots.toml`, `dns.toml` and `session-security.toml`. Their existing loading
rules are unchanged. Not every legacy field in window appearance has a Niri
equivalent: `falloffPower`, `inactiveDim`, `opacity.inactive`,
`opacity.fullscreen`, `fullscreen.decorated`, blur `enabled`, `contrast`,
`brightness` and `popupEnabled` are not activated by this change. Native
fullscreen remains opaque; popup blur rules remain explicit in the Niri template.
