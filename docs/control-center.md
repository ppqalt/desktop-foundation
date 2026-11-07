# Control Center

`Super+Z` or `scripts/control-center` toggles the left sidebar. `Super+X` still
opens btop. Bluetooth and Nothing controls remain in the existing `Super+B`
widget.

The overview contains a network summary and two audio cards: output and
microphone. Each audio card has a device-details action, mute button, short
volume slider and percentage. Device selection opens an Audio view inside the
same frame. The collapsible Applications section shows one slider per active playback stream, so multiple streams from one app remain independently adjustable. It subscribes to audio changes while expanded and verifies stream identity before setting volume. A compact Cachy Update button opens the installed `cachy-update` in Kitty.
Network details show device actions and saved connections, omitting
active non-VPN profiles already represented by a device. Network settings opens
NetworkManager's packaged `nmtui` in Kitty for adding networks or editing
credentials.

The left panel retains its desktop frame from `config/monitor.toml`: 720 logical
pixels, full output height, 14-pixel inner corners, 12-pixel outer padding and
75% background opacity. Opening/closing use the existing 240/180 ms
velocity-preserving slide. These are explicit desktop adaptations, not Google
component tokens. The right btop sidebar is unchanged.

## Material components

The Control Center uses Google's Material 3 specifications, mapped at one dp to
one Qt logical pixel before output scaling:

| Component | Specification used |
| --- | --- |
| Filled cards | 12 dp corners, 16 dp content inset, `surfaceContainerLow` |
| Small action/toggle buttons | 40 dp visual height within a 48 dp input target; 16 dp horizontal inset; 20 dp leading symbols and 8 dp icon-label gap |
| Button shapes | Full rounding at rest; 12 dp selected toggle corners; 8 dp pressed corners |
| Small sliders | 24 dp tracks, 8 dp track corners, 44×4 dp handle, 2 dp pressed handle, 6 dp handle gap, 48 dp input height |
| Device dropdowns | 56 dp filled field; 48 dp menu rows |
| Device/action lists | 56/72 dp single/two-line items with 16 dp body and 14 dp supporting text |
| Heading | Headline Small, 24/32 dp |
| Text and state | Paired Material foreground/container roles, distinct keyboard focus, on/off or selected state, and disabled treatment |

Controls read `shell/theme/Material.qml`. Google Material Symbols Rounded paths
are bundled there under Apache-2.0 with their license. Fonts use the existing
Google Sans Flex family; no icon-font package is required.

Rust preserves Matugen's complete dark role pairs in `semantic.json.material`.
The existing top-level graphite roles remain unchanged for other surfaces.
Material cards have opaque tonal fills, so their text contrast is independent
of the wallpaper behind the translucent frame. Error messages use the Material
error-container/on-error-container pair. Invalid or incomplete Material schemes
and text pairs below 4.5:1 are rejected before publication. The baseline Material
dark scheme in `theme/fallback/material.json` is used for reset and older
palette consumers.

Small shape/state effects use Google's documented fast spatial/effects Bézier
fallbacks (350/150 ms). Slider values have no interpolation: the bar and number
respond immediately, including during keyboard repeats. The standalone volume
OSD keeps its existing fades.

Sources: [buttons](https://m3.material.io/components/buttons/specs),
[sliders](https://m3.material.io/components/sliders/specs),
[color roles](https://m3.material.io/styles/color/the-color-system),
[typography](https://m3.material.io/styles/typography/type-scale-tokens),
[motion](https://m3.material.io/styles/motion/overview/specs).

## Keyboard operation

| Keys | Action |
| --- | --- |
| Arrows / WASD on actions | Move spatially to the next available control; no wrapping at an edge |
| Enter / Space | Activate an action or open a device menu |
| Left/Right or A/D on sliders | Decrease/increase volume immediately |
| Up/Down or W/S on sliders | Move to another control |
| Up/Down or W/S in open menus | Highlight a device |
| Enter / Space in open menus | Select the highlighted device |
| Escape | Close an open device menu first, then return from details, then close the panel |
| Tab / Shift+Tab | Traverse the focus chain |
| Ctrl+R | Refresh state |

Returning from details restores the initiating control's focus. Focused controls
scroll into view. WASD handling is confined to the non-text controls in this
panel; it does not install global shortcuts or intercept typing elsewhere.
The layer uses on-demand keyboard focus and does not block other windows.

## Runtime

The existing Rust backend validates and performs device operations. Audio
requests run independently of network requests; rapid volume updates coalesce.
Port descriptions provide concise overview labels while full device descriptions
and stable identifiers remain available in the device picker. Selecting a device
reroutes existing streams as well as future streams.

The panel samples state every two seconds only while open. Closing finishes the
slide, stops sampling and unloads the QML component. Explicit in-flight requests
finish before unloading. No additional daemon, permanent worker or background
animation process is added. Passwords remain in the native network editor rather
than the Control Center's arguments, logs or state files.

Isolated checks use simulated device services and an offscreen Qt window. They
cover arrows/WASD, Enter/Space, menu/back focus, immediate volume changes,
independent request queues, animation reversal and unloading without exercising
live device controls.

## Wallpaper selector

The overview's three-column grid reads `Wallpapers` inside the XDG Pictures
directory (normally `~/Pictures/Wallpapers`). To choose another directory, set
`directory = "~/My Wallpapers"` in `~/.config/desktop-foundation/wallpapers.toml`.
Absolute paths are supported. Image names are
shown on hover and exposed to accessibility tools; the active wallpaper has a
checkmark. Arrows/WASD navigate, Enter/Space applies, and Tab joins the existing
focus chain. A partial row retains the same tile width.

Rust inventories PNG, JPEG, WebP, GIF, BMP, TIFF, AVIF, TGA, QOI, PNM, PBM,
PGM, PPM, PAM, ICO, HDR, OpenEXR and Farbfeld images and caches small, rounded
PNG previews. JPEG XL and HEIC/HEIF are also available when the installed FFmpeg
can decode them. Formats unsupported by the wallpaper renderer are converted to a
cached PNG while the original remains the selected source. QML displays previews asynchronously without shader masks or
per-tile offscreen effects. Changed/deleted images invalidate their cached
previews. The directory is read when entering the overview, without a watcher or
polling daemon.

Selection uses the native wallpaper/theme transaction. The checkmark moves only
after success; a failed change keeps the old theme and selection. Closing hides
the panel immediately through its existing animation, while an explicit apply
finishes before the panel instance is released. No worker remains afterward.

`cargo test --locked --manifest-path native/foundation/Cargo.toml --test wallpapers`
checks discovery, preview caching/visible pixels, three-column geometry, keyboard
navigation, failure handling and closing during a fake application. It uses an
offscreen QML host and never changes the running desktop's wallpaper.
