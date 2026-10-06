# Bluetooth

**Super+B** opens paired devices on the focused output. The first paired device
is selected when the list arrives, unless you have already selected something.
Up/Down, Tab/Shift+Tab, Home/End, Page Up/Down and scrolling move selection.
Enter/Space or a click activates the displayed action. Escape or an outside click
closes the popup. Device identity preserves selection across state updates.

Connect and Disconnect wait for BlueZ confirmation. A successful connection closes
after playback setup; failures remain visible. Connected supported Nothing/CMF
devices offer **Controls**, with Disconnect inside that page.

The header switch powers all detected adapters. Turning it off disconnects their
devices. Power-on clears the Bluetooth software rfkill block, sets BlueZ `Powered`
and checks the result. Adapter recovery retries share a five-second budget after
discovery/unblock; hardware blocks appear as an error. The switch displays observed
power state and accepts one request at a time.

## Devices and playback

Quickshell watches paired/bonded BlueZ devices across adapters. The list is sorted
by name/address. Secondary text shows connected state, reported battery and the
codec reported by PipeWire. Unavailable battery or codec information is omitted.
Nothing/CMF probes can add separate left, right and case batteries when reported.

The Rust backend invokes BlueZ connection methods, validates pairing and radio
state, and checks the final Connected property. Playback setup subscribes to
PipeWire-Pulse events before taking snapshots. Relevant card/sink/server events
drive updates within a 12-second budget, with eight-second command limits.

Available A2DP profiles rank **LDAC, AAC, then advertised priority**. Failed or
unavailable choices fall through. After confirming the matching sink/profile and
codec, the helper changes the default playback output. An audio setup warning can
accompany a successful Bluetooth connection. WirePlumber handles stream routing,
per-device volume, disconnect fallback and microphone-driven headset switching.

`audio/wireplumber/60-desktop-foundation-bluetooth.conf` sets quality preference,
enables headset autoswitch and disables persistent headset-state storage.
Connections made outside the popup follow WirePlumber's policy. Popup connection
notifications use the top-center provider and a two-second expiry.

## Management and commands

**Manage…** opens Blueman for scanning, pairing, forgetting devices, passkeys,
services and adapter administration. Its applet starts on demand with the manager.
Installation supplies BlueZ, Blueman, PipeWire-Pulse and WirePlumber and enables
`bluetooth.service`; deployment journals the user policy and autostart overrides.

```sh
scripts/foundation bluetooth power on
scripts/foundation bluetooth connect /org/bluez/hci0/dev_XX_XX_XX_XX_XX_XX
scripts/foundation bluetooth disconnect /org/bluez/hci0/dev_XX_XX_XX_XX_XX_XX
scripts/foundation bluetooth codecs /org/bluez/hci0/dev_XX_XX_XX_XX_XX_XX
scripts/foundation bluetooth codec /org/bluez/hci0/dev_XX_XX_XX_XX_XX_XX --codec sbc_xq
```

Use the device's actual BlueZ path. SBC and SBC XQ select host playback only when
the card advertises those profiles. The supported Ear (3) Quality page exposes them
alongside its separate firmware preferences. See [device controls](nothing-controls.md).

References: [Quickshell BluetoothDevice](https://quickshell.org/docs/v0.3.1/types/Quickshell.Bluetooth/BluetoothDevice/),
[WirePlumber settings](https://pipewire.pages.freedesktop.org/wireplumber/daemon/configuration/settings.html),
[Bluetooth configuration](https://pipewire.pages.freedesktop.org/wireplumber/daemon/configuration/bluetooth.html).
