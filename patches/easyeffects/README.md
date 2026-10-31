# EasyEffects monitoring-volume API

Optional patch for upstream **8.3.1**, commit
`b4390ab3fec06fcfbd6dcf2c2fa1d12bca23bc68`. This is an explicit local extension,
not an API supplied by the unmodified EasyEffects package.

The upstream socket supports monitoring on/off but exposes plugin properties only
through `set_property`/`get_property`. Monitoring gain belongs to `DbStreamInputs`,
not a plugin, and the monitor's DSP gain is not a writable PipeWire volume.

This patch adds:

- `get_microphone_monitoring_volume\n`: linear gain with 17 significant digits.
- `microphone_monitoring_volume:GAIN\n`: finite linear gain in [0, 4], returns
  `ok\n` or `error_invalid_monitoring_volume\n`.
- An `ok\n` acknowledgment for the existing `microphone_monitoring:0|1\n` setter.

Only `listenToMicVolume` and the already-existing monitoring toggle are affected.
No effects, presets, recording gains, application capture routes or device
preferences are changed by the patch. Existing clients may ignore the added ACK.
The range includes a pre-existing gain of 2 so it can be restored exactly;
Foundation's activation target is separately restricted to [0, 1].

Build this optional package as the normal user, with the listed build dependencies
installed, then install the resulting package through pacman:

```sh
cd patches/easyeffects
makepkg
sudo pacman -U ./easyeffects-8.3.1-1.2-x86_64.pkg.tar.zst
```

A running EasyEffects process keeps its old executable until it exits. Quit and
reopen it once after installing; Transparency Mode itself never opens its GUI or
restarts it. `scripts/transparency check` verifies the API without changing audio.
A future stock package update may remove the extension; Foundation fails before
changing audio if it cannot query the monitoring-volume API.

To return to the stock package, install the stock package from the package cache
(or the repository), then quit/reopen EasyEffects once. Turn Transparency Mode off
first and ensure its recovery journal is cleared.

Upstream reference:
[local server](https://github.com/wwmm/easyeffects/blob/v8.3.1/src/local_server.cpp),
[monitor gain](https://github.com/wwmm/easyeffects/blob/v8.3.1/src/microphone_monitor.hpp),
[monitoring settings](https://github.com/wwmm/easyeffects/blob/v8.3.1/src/contents/kcfg/easyeffects_db_streaminputs.kcfg).

`tests/test-easyeffects-monitor-api /absolute/path/to/patched/easyeffects` verifies
these commands against a private PipeWire graph and a private D-Bus with no
service activation directories. It does not use the host microphone or audio
session. The fixture checks invalid gains and on/off acknowledgments as well as
monitoring-volume readback.
