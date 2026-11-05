# Transparency Mode

`scripts/transparency toggle` toggles room listening through the selected
microphone with application ducking. Insert is assigned to media navigation.

**Shift+Insert** toggles monitoring without changing application volumes. Press
the same shortcut again to restore the previous monitoring state. Switching
between the two modes restores the old mode before starting the new one.

## Required EasyEffects extension

Stock EasyEffects 8.3.2 exposes monitoring on/off but **not external monitoring
volume control**. Install the reviewed optional
[monitoring API patch/package](../patches/easyeffects/README.md), then quit/reopen
EasyEffects once. `scripts/transparency check` checks the API and devices without
changing audio. The feature fails before changing audio if the API is absent.
It never starts/restarts EasyEffects or opens its window.

## Operation

The Rust owner snapshots monitoring enablement, its exact linear gain, and each
application's per-channel integer playback volume. It verifies a PipeWire route
from the selected microphone through EasyEffects' dedicated microphone-monitor branch to the
current physical playback device **before** ducking applications. Muted microphone
input is an error; it is never unmuted. Existing effects and device selection
remain unchanged. This feature adds no processing and does not change Discord,
source recording gains/mutes, master output volume, presets or application routes.

Playback is reduced to 25% of its prior volume; monitoring defaults to 0.5 linear
gain. Four short steps soften transitions. Exact latency and sound quality depend
on the existing PipeWire graph, effects and device buffers; fixture tests do not
measure audible latency. No GUI is launched. Feedback reuses the existing
non-focus-stealing Material readout with “Transparency On/Off”, semantic colors,
Google Sans Flex and its existing fade; no new window or permanent widget.

Repository defaults: `config/transparency.json`. Optional full user override:
`$XDG_CONFIG_HOME/desktop-foundation/transparency.json` (default `~/.config`).

```json
{
  "microphone_match": "",
  "playback_factor": 0.25,
  "monitoring_gain": 0.5,
  "fade_ms": 80
}
```

An empty `microphone_match` selects the system's default physical input. Set it
to a distinctive part of a device name to retain an explicit selection; it must
match exactly one physical input. When the default input is virtual, a single
detected physical microphone is selected automatically; multiple candidates
require an explicit selection. Monitoring still requires the verified EasyEffects route.

Playback factor is bounded to [0, 1], monitoring gain to [0, 4], and fade to 0–500 ms. Monitoring gain is linear,
not a recording-gain or master-volume percentage. The prior gain (including a
pre-existing value above 1) is restored rather than replaced with this default.

## Ownership and restoration

- Each stream is identified by Pulse index **and PipeWire object serial**, with
  its channel-map order retained. New streams are captured/ducked while active;
  closed streams are dropped, and reused indexes cannot inherit old volumes.
- A manual per-application volume change releases that stream for this activation.
  Its newer value is neither re-ducked nor overwritten on exit. Manually changed
  monitoring state/gain is also preserved by value comparison.
- Every write records its intended value before applying it. Restoration accepts
  either the previous confirmed value or that pending value, covering a crash on
  either side of the write. Unknown/newer values are left alone.
- A restarted EasyEffects instance is read afresh. Only still-owned settings are
  restored, including temporary settings reloaded by EasyEffects' own autosave.
- A failed monitor setup rolls back before ducking. Later errors restore owned
  playback volumes even if EasyEffects is unavailable; unresolved monitoring
  recovery remains journaled and is retried on the next start or explicit recovery.

A user systemd datagram socket queues monitoring requests during transitions, shutdown
and startup. It activates one Rust owner. That owner and `pactl subscribe` exist
only during the activation and exit after deactivation. Off leaves only a kernel
socket, with no polling process/CPU cost. Active mode waits for socket/PipeWire
events; finite route-settle checks and transition steps are not idle polling.
Client-only graph events are ignored so diagnostics do not create an event loop.
The existing Control Center/volume lock protects stream writes.

`ExecStopPost` runs idempotent recovery after owner failure or session stop.
Private journal: `$XDG_STATE_HOME/desktop-foundation/transparency/journal.json`.
If recovery cannot finish, keep that file and run `scripts/transparency recover`
after EasyEffects/PipeWire is available. Never delete it to silence an error.

A power failure cannot execute recovery immediately. Likewise, an unavailable
PipeWire server cannot accept volume writes. Recorded recovery is retried when
the feature is next invoked; this is not a guarantee of restoration while the
audio server is inaccessible.

## Verification and rollback

Rust fixtures exercise both initial monitoring states, exact unequal-channel
volumes, manual conflicts, missing devices/API/routes, stream lifecycle, repeated
transitions, interrupted writes and restarted/unavailable EasyEffects. QML fixtures
exercise the shared temporary notice and immediate volume readout. Live audible
latency, comfort and physical key acceptance require the actual patched process.

Turn the mode off before removing it. Explicit recovery is safe to rerun.
Remove the Shift+Insert binding to stop intercepting that shortcut. Stop the unused
socket with `systemctl --user stop desktop-foundation-transparency.socket` after
recovery; existing deployment rollback restores owned configuration/units.
See the patch README to restore the stock EasyEffects package.

References: [upstream socket protocol](https://wwmm.github.io/easyeffects/user_interface/local_server.html),
[Material snackbar guidance](https://m3.material.io/components/snackbar/overview).
The existing desktop readout is a documented adaptation, not a claim of an
exact snackbar implementation; Google's overview requires JavaScript.
