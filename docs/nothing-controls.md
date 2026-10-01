# Native Nothing / CMF controls

Super+B is still the Bluetooth entry point. A connected device advertising the
Nothing control UUID offers **Controls**. Enter/click opens the native page;
Escape/Back returns to paired devices, then Escape closes. Wheel/Up/Down change
selection only. Left/Right adjust a selected value; Enter cycles presets or toggles.
Custom EQ, gestures, Find and information have secondary pages. Find warns to remove
the earbuds and sends a three-second ring followed by stop, including on normal
Back/outside-click cancellation. Ring has no readable device-state command in the
references; it is an action, not a confirmed setting.

The card, rows, selection, typography, colors, blur namespace and entrance animation
reuse the existing shell. Normal Bluetooth connection/disconnection and Blueman
fallback remain unchanged. Unsupported identities/protocol failures return to the
list with an explanation and generic Disconnect; closing/reopening permits retry.

## Backend and source

`native/nothing` builds `foundation-nothing`, a Rust JSON-lines stdio helper. The
shell calls `scripts/nothing-backend`; nothing is installed as a system daemon or
HTTP service. A finite `--discover` process reads paired devices' advertised UUIDs
once when the list opens (no name/hostname/address assumption). Sequential finite
`--battery` probes identify connected supported devices and populate separate left,
right and case percentages in the paired list. Missing components stay absent;
reports expire after two minutes and are discarded on disconnect. Controls reports
refresh the same cache when returning to the list. No background polling. Controls starts a
separate helper lazily, registers a BlueZ ProfileManager1 **client** profile for
`aeac4a03-dff5-498f-843a-34487cf133eb`, asks Device1.ConnectProfile, and accepts its
RFCOMM descriptor. BlueZ resolves SDP/channel; no hardcoded channel or sdptool.
The requested device must be paired, connected and advertise the service; other
incoming profile connections are rejected. The serial response identifies the model
through the upstream SKU/prefix table. Unknown models keep generic Bluetooth.

See [NOTICE](../native/nothing/NOTICE.md) and
[GNU AGPL license](../native/nothing/LICENSE). The model database is kept separately
in `src/upstream`; framing and setting codecs explicitly credit Daan Hessen's
[earctl](https://github.com/DaanHessen/earctl) and protocol research from
[ear-web](https://github.com/radiance-project/ear-web). Commits are pinned in NOTICE.
No web UI, CSS, icons or Nothing-owned visual assets were copied. The helper's
complete source, lockfile and license must accompany distribution.

## State boundaries and reliability

- **BlueZ** supplies paired/connected state, adapter and RFCOMM transport.
- **Earbud protocol** supplies model, firmware, individual batteries and controls.
- **PipeWire** supplies the current playback codec/profile. LDAC is never inferred
  from a device capability or control-channel connection. Audio selection remains
  the existing bounded connection helper/WirePlumber policy.

Packets use `55 60 01`, little-endian command/length, operation id, payload and
CRC16/Modbus. Parsing handles fragmented/coalesced input and resynchronizes past
corrupt frames, with a 4096-byte payload bound. Requests have an absolute two-second
read deadline, so asynchronous notifications cannot keep a request alive forever.
Connect retries transient BlueZ busy/previous-connection errors with bounded
exponential backoff, within a ten-second bound; startup also has a 35-second UI bound. Writes and
UI commands are bounded. Ordinary responses correlate command and operation id;
unsolicited battery/ANC notifications update state while queries are in progress.

Controls appear only for a recognized model and a valid feature response. Known
model restrictions gate probes. Setting writes are followed by the corresponding
read query, plus related EQ/Bass/listening state where applicable; equality is required for confirmation. Actual reported state is always
shown even when it differs from the request. Readback failure invalidates that
setting until reopening, rather than pretending the requested value was applied.
Battery response IDs 2/3/4/6 represent left/right/case/headphones, with a charging
bit. Missing components and invalid percentages are null. A new battery packet
replaces the previous report; no stale missing component is carried forward.
Asynchronous packets update the page. After two minutes without a battery report,
values become unavailable; Refresh battery explicitly queries, without polling.
Closing destroys the page/state/helper; reopening performs fresh queries.

BlueZ device events and RFCOMM EOF detect disconnect/restart, return to the generic
list and discard control values. Reconnection uses the established connect/audio
flow, then fresh Controls initialization. A transient failure disables Controls
only for that popup lifetime. No resident Nothing process or once-per-second CLI
invocations. SIGTERM/stdio-close completes normal cleanup; explicit process exit
avoids Tokio's blocking-stdin reader delaying shutdown. The UI enforces a
1.5-second forced-shutdown fallback and returns to the generic list on command
timeout or helper crash. Numeric delegate models preserve scrolling across live
updates. Keyboard/wheel navigation ignores stationary-pointer hover; page Back
restores the parent selection. Tab and Shift+Tab both navigate.

## Implemented controls and limits

- Noise control: off, transparency, ANC; known model maps restrict available levels.
  B173 validates low/high/medium/adaptive. Other models are conservative and untested.
- Standard EQ: Balanced, More voice, More treble, More bass and Custom where queried.
  CMF listening-preset command is separate; untested models display device preset IDs.
- Three-band custom EQ: Bass/Mid/Treble, -6…+6 dB, actual device readback. Saving bands
  and selecting Custom are explicit separate actions.
- Bass Enhance: toggle and levels 1…5, with response validation/readback.
- In-ear detection and low latency: supported query responses only, actual toggles.
- Gestures: reported left/right double/triple/hold/double-hold slots, preservation of
  common/slot bytes, constrained actions and full-table readback. Case gestures and
  unknown slots remain untouched. Gesture names are neutral for tap/pinch models.
- Find: known earbud models except B181 (its protocol rings both sides), only sides with valid battery reports; explicit sound
  warning and bounded start/stop. Actual audible operation is not yet validated.
- Information: detected model code, firmware, Bluetooth address, advanced-EQ limit.

The references expose advanced-EQ **enabled status**, not a reliable advanced-band
read/write codec. This is reported in information, with no fake advanced editor.
Spatial audio is also set-only in the references and is not presented as a verified
toggle. Super Mic, personalized ANC, fit tests and case controls are not implemented.
Other models' capabilities remain based on upstream mappings plus valid queries;
only B173 has been hardware-validated here.

## Installation, diagnostics and rollback

`./scripts/install` installs rustup/GCC/pkgconf/D-Bus alongside existing Bluetooth
packages, provisions the pinned minimal Rust 1.98.1 toolchain and runs
`./scripts/build-nothing` with `cargo --locked --release`. No manual upstream clone.
`--no-packages` still builds and requires those dependencies/toolchain already present.
Build output stays ignored inside `native/nothing/target`; no user config or helper
service is added. The normal reversible shell deployment selects this source.

```sh
./scripts/build-nothing
./scripts/nothing-backend --discover
./scripts/nothing-backend /org/bluez/hciN/dev_XX_XX_XX_XX_XX_XX
# JSON lines on stdin: {"action":"refresh"}, {"action":"close"}
# e.g. {"setting":"anc","value":7}; replies show device state and confirmation
quickshell ipc --path shell call foundation bluetoothStatus
cargo test --locked --manifest-path native/nothing/Cargo.toml
```

Close Controls before using the diagnostic helper (one control profile at a time).
Do not publish diagnostic output containing Bluetooth addresses or serials.
Rollback the extension's commits and rebuild/reload the shell; normal Bluetooth
continues through BlueZ/Blueman. Full `scripts/uninstall` restores managed configs
and stops the shell; no Nothing service, autostart or private setting cache remains.
Build artifacts/toolchain are not removed as part of configuration rollback.

## Validation on Tops, 2026-10-01

Detected **Nothing Ear (3), B173**, firmware **1.0.1.69**. Separate left/right levels
and unsolicited battery changes observed. Case not reported; no case value invented.
ANC low/high/medium/adaptive/transparency/off all changed and read back. EQ presets
and Custom selection changed/read back. Custom bands changed to [-2,1,2] and read
back. Bass levels 1/3/5 and enabled/disabled, in-ear and low-latency toggles changed
and read back. Left double gesture changed and full table read back. **All original
settings restored.** Playback remained LDAC from PipeWire; capture routing unchanged.

Actual Super+B, Enter, wheel, arrows and subpage/Controls/list Escape navigation
passed via kernel input events; screenshot inspected against the existing design.
Real Device1 disconnect while Controls was open returned to the generic list;
reconnect and Controls reopening gave fresh state. Generic non-Nothing hardware
was not available for a physical test; its existing action path and regression tests
are retained. Earbud case sleep/wake and a new BlueZ service restart are pending
physical/authenticated user actions; earlier Bluetooth restart validation does not
count as validation of this new extension. Find was not activated audibly.

Release helper measured **2874 KiB PSS / 5716 KiB RSS**, zero CPU ticks across a
three-second idle sample. Shell PSS was 152251 KiB hidden, 170307 KiB with Controls,
155243 KiB after close (allocator/service caches retain a small amount). Helper
exited on close: **no resident Nothing process**. Eight Rust tests cover framing,
missing/invalid batteries, safe decoding, custom EQ, correlation, failed readback
and absolute timeout under continuous asynchronous events. Repository static
checks, 28 Python tests and live Niri adapter regression passed.

The opt-in `python3 tests/live_nothing_surface.py` exercises the actual kernel-input
user surface on connected Ear (3): mouse, wheel, arrows, Tab/Shift+Tab, all ANC/EQ
presets, setting readbacks, page/Back behavior, rapid open/close, frozen/crashed
helpers, UI Disconnect/reconnect and fresh battery state. It restores original
settings in a final independent helper pass and checks capture routing. It moves
the pointer and temporarily changes settings; run manually, never in normal CI.
Find sound is deliberately excluded. Three consecutive UI reconnections also
passed with LDAC and separate battery values retained.

Quickshell diagnostics use the installed `adw-gtk3-dark` palette through the GTK
Qt platform theme, scoped by `scripts/run-shell` to the shell process. This fixes
white reload-error windows without suppressing errors or changing custom QML
surfaces. An isolated intentionally invalid config confirmed dark background,
readable error text, log access and the retained error border. No extra daemon or
Qt theme configuration is needed; the existing GTK theme dependency is reused.

The vendor channel permits one control client at a time. An active ear (web)
Web Serial connection in Brave reproduced BlueZ `br-connection-create-socket`;
the popup now explains that other earbud-control applications should be closed.
It leaves that application and audio session alone and keeps the generic list usable.
