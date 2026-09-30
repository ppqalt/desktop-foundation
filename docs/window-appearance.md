# Niri visual treatment

Normal active/inactive window opacity is equally 90% for the user's trial. Native
Niri blur, 14 px corner geometry, muted 1 px border and soft depth translate shared
config/window-appearance.lua. Native Kitty launches override background_opacity
with the shared value; a Kitty rule keeps whole-window opacity at 1 so text stays
opaque. Existing Kitty windows need a fresh launch to use the new override.

Xray blur samples background layers, not other windows. A flat gray background has
no detail to blur. No wallpaper feature has been introduced. Popup background blur
is disabled; Niri opacity itself also affects popups. Fullscreen styling is native
Niri behavior; the Hyprland-specific fullscreen 1.0 rule is not a portable Niri
state matcher. Niri 26.04 IPC cannot report authoritative fullscreen state.

The sections below document retained Hyprland implementation and historical QA;
Niri's reference implementation and validation are in niri-port/niri-validation.

# Graphite window treatment

config/window-appearance.lua centralizes compositor-neutral window design intent.
It is a plain data table, without hl.* calls, monitor names, GPU choices or rule
selectors. compositor/hyprland/appearance.lua translates it to native Hyprland
configuration; no focus-event script, polling or extra resident process is needed.
Surface card/typography/color tokens stay in shell/theme/Theme.qml. The previously
unused QML blurRadius token was removed so it cannot suggest a second global blur
setting. Window colors match the approved graphite/blue-grey shell palette.

Current values:

| Intent | Value |
|---|---|
| Corners | 14 logical px |
| Window separation / desktop edge | 12 / 18 logical px |
| Background blur | radius 7, 3 quality passes, strength 1.0 |
| Blur color treatment | noise 0.012, contrast/brightness 1.0, saturation boost 0.06 |
| Soft shadow | #080b10, range 18 px, downward offset 5 px, falloff power 3 |
| Active / inactive shadow opacity | 0.42 / 0.12 |
| Focus edge | 1 px, active #485669 / inactive #2b323d |
| Active / inactive / fullscreen window opacity | 0.90 / 0.90 / 1.0 |
| Inactive dim | 0.0 (disabled) |
| Fullscreen | no compositor border, shadow or rounded clipping |

Shadow differentiation was inspected first with borders disabled. On this dark
wallpaper it gave depth but was too quiet to identify keyboard focus reliably.
A thin muted edge gives the active window an immediate cue without changing
application content opacity or adding dimming/glow. Hyprland animates its native
active/inactive shadow and border states using the existing shortened animations.
Opaque applications stay opaque; blur is visible only where an app supplies
transparency. The preview applications deliberately supply transparency so the
quality can be judged; no application configuration was changed.

Hyprland has a global blur kernel and per-window enable/disable effects, not a
useful per-focus radius/passes control. Stronger blur on focus would affect only
transparent apps and can make wallpaper detail jump when focus changes. We use
depth and the thin edge instead. There is no independent native blur-alpha slider:
the shared strength value scales the radius on this backend (0 disables it).
Window opacity remains a separate control. Thus radius/pass/strength is tunable,
but strength is not represented as an invented Hyprland blur-opacity property.

## Popup and fullscreen safeguards

The existing floating-XWayland no_blur rule remains after appearance translation.
It prevents transparent popup shadow margins from becoming blurred rectangles.
It also excludes ordinary floating XWayland windows from compositor blur; their
shadows and focus edge still work. Native popup background blur is off. Client
menu shadows remain client-owned. A fullscreen rule matches internal state 2,
so scrolling-layout maximize is not accidentally treated as true fullscreen.
Native fullscreen geometry removes layout gaps; compositor border, rounding and
shadow are explicitly disabled there. Client-side decoration remains app-owned.

Live inspection covered native Wayland and XWayland preview windows, two scrolling
columns and focus changes, floating windows, true fullscreen on both paths, and
native/XWayland popup surfaces. The real ChatGPT XWayland context menu also renders
without the old rectangular blur margin. Fullscreen snapshots filled the output
at origin 0,0 with 1920x1080 content; these dimensions are observations, not config
constants. Preview processes were stopped and the original workspace/focus restored.
All three profile config checks and existing static checks pass. No cosmetic tests
were added. Other monitor/GPU combinations and a real Niri session remain untested.

## Native implementation boundaries

Desired semantics are rounding, transparent background softness, active/inactive
depth, focus emphasis and clean fullscreen. Pixel-for-pixel renderer equivalence
is not the contract. Niri should translate these into native shadows, geometry
corners, a restrained focus ring/border and supported background effects. Its
shadow softness is not Hyprland's shadow range/falloff; visually tune the mapping.
Niri's newer background-effect support is version-dependent. Keep unsupported
options explicit rather than adding a Hyprland emulation process. See
[Hyprland decoration options](https://wiki.hypr.land/0.56.0/Configuring/Basics/Variables/),
[Niri layout and shadows](https://github.com/niri-wm/niri/blob/main/docs/wiki/Configuration%3A-Layout.md)
and [Niri window effects](https://github.com/niri-wm/niri/wiki/Window-Effects).

First live translucency trial: normal active and inactive windows now both use
0.97 opacity so opaque app backgrounds expose the existing blur. A finite blur
on/off comparison on real ChatGPT and Kitty backgrounds confirms the blur
changes their rendered pixels. Fullscreen remains 1.0, and popup safeguards are
unchanged. Native application-background transparency tuning is a later step;
this trial deliberately changes only the normal-window opacity.
