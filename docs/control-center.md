# Control Center

`Super+Comma` or `scripts/control-center` toggles the left sidebar. The reserved
control UI shortcut is now active; existing shortcuts, including `Super+X` for
btop and `Super+B` for Bluetooth, retain their functions.

The panel reads `config/monitor.toml` through the Rust backend when opened. The
same settings supply both sidebars: 720 logical pixels wide, full output height,
14-pixel inner corners, 12-pixel padding and 75% background opacity. The Control
Center attaches flush left, with only the right corners rounded. Niri supplies
the same graphite blur and shadow with mirrored corner geometry. QML reproduces
the btop controller's cubic Hermite motion, including velocity-preserving
reversals: 240 ms open and 180 ms close at rest. No space is reserved for a bar.

Controls include:

- Audio output and microphone selection, volume and mute. Selecting a device
  reroutes existing playback/capture streams as well as future streams.
- Bluetooth power and paired-device connect/disconnect, with a link to the
  existing pairing and Nothing controls.
- Networking and Wi-Fi radio switches, Ethernet/Wi-Fi device connect/disconnect,
  and saved NetworkManager connections, including VPN profiles.
- Network settings opens NetworkManager's packaged `nmtui` in Kitty for adding
  Wi-Fi networks or editing credentials. Passwords do not pass through the
  Control Center's arguments, logs or state files.

Tab/Shift+Tab move through controls. Arrow keys adjust sliders and select device
options; Enter/Space activate buttons. Escape closes a device dropdown first,
then the sidebar. Ctrl+R refreshes status. Focused controls scroll into view. The
layer uses on-demand keyboard focus, so it does not block interaction with other
windows while open.

Quickshell owns presentation; the existing Rust backend validates and performs
all device changes. Audio requests use the same volume lock as keyboard volume
changes. They run independently of network/Bluetooth requests; rapid slider
updates coalesce to the latest value. Volume bars update immediately without
value animations.

The panel samples audio/network state every two seconds only while open, with
finite service requests and no automatic Wi-Fi discovery. Bluetooth state comes
from Quickshell's native BlueZ model. Unavailable services/devices are shown
without preventing other sections from working. Network controls use the existing
NetworkManager service; the installer includes its package, but the panel does
not replace or automatically enable a different network stack.

The sidebar is a lazy QML component inside the existing shell. Closing finishes
the slide, stops sampling and unloads the component. An explicitly requested
in-flight operation is allowed to finish before unloading; it is not interrupted
halfway through a device change. There is no new service, permanent worker or
hidden panel process. The btop sidebar's configuration and lifecycle are unchanged.
