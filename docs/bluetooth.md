# Paired Bluetooth popup

**Super+B** toggles the lazy Quickshell surface. Up/Down, Tab and the shared mouse
wheel handler change selection only; click/Enter activates the clearly labeled
Connect/Disconnect action. Escape or outside click closes, including during a
request. Successful connection closes after actual BlueZ confirmation and bounded
playback setup. Failure stays open with a concise inline message.

The popup uses the existing SurfaceCard, ApplicationRow, Keycap, SelectionWheel,
shared colors, typography, outlined selection, icon tiles, shadow and entrance/exit
animations. Portable local device icons avoid depending on the host icon theme.
Connected state, negotiated codec, and reported battery are compact secondary text.
Devices are sorted stably by name/address rather than maintaining a second usage DB.

The header switch turns Bluetooth on/off through native BlueZ adapter properties.
Click it, or navigate upward from the first device and press Enter. Wheel and Tab
navigation include the switch. It affects all detected adapters; turning it off
disconnects Bluetooth devices. The switch reflects reported power state, prevents
overlapping requests, and reports failure after five seconds (including airplane
mode or unavailable adapters). It does not start another daemon.

## State and actions

Quickshell 0.3.1 `Quickshell.Bluetooth` watches BlueZ ObjectManager/properties on the
system D-Bus. Only paired/bonded devices are shown, across discovered adapters;
trusted-only scan results are not treated as evidence of an existing pairing.
No discovery, pairing, forgetting, PIN agent, adapter polling or bluetoothctl.
`Quickshell.Services.Pipewire` and PwObjectTracker read live sink properties for
codec/profile reporting. Battery is BlueZ Battery1 through Quickshell, only shown
when supplied. Missing battery/codec information is omitted, never invented.

Generic Bluetooth does not require a native backend. Quickshell's device connect API returns void and
has no detailed error signal; its PipeWire API does not expose card profile lists.
A finite `scripts/bluetooth-action.py` fills these two gaps: `busctl` invokes BlueZ
Device1 methods directly over D-Bus, awaits method completion and verifies Connected.
It checks pairing and blocked/powered state before connection. Errors are translated
rather than dumping D-Bus names. After playback setup it verifies the device remains
connected. Disconnect is similarly confirmed. Cancellation cleans child processes.

## Playback policy

The existing PipeWire-Pulse compatibility server exposes JSON cards/profiles/sinks
through `pactl`. On intentional connection to a device advertising Audio Sink UUID,
the helper subscribes to server events before its first snapshot, then waits on
those events with a single 12-second deadline. There is no repeated timer polling.
All addresses, card/sink names and profile IDs come from live state.

Available A2DP profiles rank **LDAC, AAC, then the remaining advertised priority**.
Both ID and advertised codec description are considered: this machine's LDAC
profile is named `a2dp-sink`, not `a2dp-sink-ldac`. Unsupported or failed selections
fall through; lack of audio readiness never marks a genuinely connected device as
unconnected. A warning explains when playback could not be switched automatically.
Once the selected sink exists with the expected codec, only its default playback
output is changed. No capture target, volume or stream restoration policy is set.
WirePlumber handles stream routing, per-device volume, fallback on disconnection,
and temporary microphone-driven HFP/HSP switching back to playback afterward.

`audio/wireplumber/60-desktop-foundation-bluetooth.conf` uses supported 0.5 settings:
quality preference, autoswitch enabled, headset-state persistent storage disabled.
It does not remove codecs or globally disable device route/profile restoration.
Only intentional popup connections explicitly rank codec profiles; connections made
elsewhere follow native WirePlumber policy. No app-specific headset hacks.

Success notification uses the existing top-center provider with 2000 ms expiry.
If known, it includes the actual negotiated codec and BlueZ battery percentage.

## Installation and fallback

`scripts/bootstrap` installs bluez, bluez-utils, blueman, pipewire-audio, pipewire-pulse,
wireplumber and libpulse, then enables bluetooth.service. `scripts/install` includes
that step even with `--no-greeter`. User deployment journals the WirePlumber fragment
and Niri-only Blueman XDG autostart override for rollback.
Blueman remains responsible for scanning, pairing, forgetting, passkeys, radio
settings, services and detailed administration. Manage… opens the normal manager.
Its on-demand applet is retained because the manager relies on it; no duplicate
applet is started by this popup. An already-open manager/applet is left alone.

## Validation on Tops (2026-10-01)

Real paired headset tested through the actual Super+B chord and synthetic input:
mouse wheel forward/reverse and Up/Down change selection without changing connection;
Enter disconnects with a live state update; Enter reconnects, chooses LDAC, routes
playback and closes; reopening shows LDAC and 90% reported battery. Escape unloads
the popup. A real BlueZ restart was performed by the user; the existing Quickshell
process recovered paired/connected state without restarting the shell. Playback
and capture defaults were checked independently. The adapter-off state and helper
error mapping were checked. A later attempt to reconnect the real disconnected
headset failed: the popup stayed open with a concise on/nearby hint, and Escape
unloaded it afterward. No pairings or account data were modified.

A fresh-shell measurement showed about 8 MiB additional PSS while open (12 MiB
RSS); after hiding, memory returned to the prior range. There is one existing
Quickshell process, no resident action helper/subscriber, and no popup object when
hidden. Native service registries may remain cached inside Quickshell after first
use. Tests also verified that Blueman Manager activates exactly one temporary
applet and stops it on exit; the prior permanent applet was stopped after its
manager had closed. Niri-only autostart suppression is deployed.

Unit tests cover hard-killed helper child cleanup as well as codec ranking including generic LDAC profile IDs, unavailable
profiles, failed-codec fallback, dynamic address/path matching, playback-only
commands and readable failure mapping. Native Niri/Hyprland parser and QML checks
are part of `scripts/check`; portable tests run via `scripts/test`.

WirePlumber microphone autoswitch is delegated to its documented native policy;
a real microphone recording/return cycle has not been validated in this pass.

References: [Quickshell BluetoothDevice](https://quickshell.org/docs/v0.3.1/types/Quickshell.Bluetooth/BluetoothDevice/),
[WirePlumber 0.5 settings](https://pipewire.pages.freedesktop.org/wireplumber/daemon/configuration/settings.html),
[WirePlumber Bluetooth configuration](https://pipewire.pages.freedesktop.org/wireplumber/daemon/configuration/bluetooth.html).

## Nothing / CMF extension

Connected supported devices offer Controls rather than immediate Disconnect. The
Controls page retains an explicit Disconnect action. The generic list/connection
helper remains the same. See [native controls, licensing and validation](nothing-controls.md).
