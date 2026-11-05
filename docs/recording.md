# Recording

Install the optional `gpu-screen-recorder` package, or select `--extra recording`
in the installer. Foundation owns recording through its Rust backend; GPU Screen
Recorder handles capture and encoding. No OBS instance or replay buffer is needed.

- **Ctrl+Shift+F12:** start with separate desktop and microphone tracks.
- **Ctrl+Shift+Alt+F12:** start with no audio.
- **Ctrl+Shift+F12 stops any active recording**, including a silent recording.
  The Alt variant also stops an existing recording; neither switches its audio mode.

The clock appears automatically during capture. Its red indicator starts only
after GPU Screen Recorder reports its first captured frame. It remains visible
while stopping and while checking the completed container. It clears only after
the recorder exits successfully and `ffprobe` confirms a nonempty video with the
expected audio tracks and readable duration. This checks container finalization,
not a full decode of every frame.
The normal clock shortcut can still hide or show it while recording, without
stopping capture. If left visible, the indicator stays through finalization.

On failure, a warning remains in the clock and a notification explains the error.
Partial files are retained. Inspect `scripts/record status`; use
`scripts/record acknowledge` to dismiss the warning after inspection.

## Settings

Defaults: [config/recording.toml](../config/recording.toml).
Override selected values in `$XDG_CONFIG_HOME/desktop-foundation/recording.toml`
(normally `~/.config/desktop-foundation/recording.toml`). Sorted fragments in
`recording.d/*.toml` load before that file. `scripts/record plan` prints effective
settings, sources, the focused display and the exact proposed command without
starting capture. `scripts/record plan --silent` shows the no-audio variant.

```toml
resolution = "auto"
fps = 0
max_fps = 60
audio_bitrate_kbps = 512
```

`auto` resolution passes through the actual captured source's dimensions. No
monitor name, GPU model or rendering device is fixed. Automatic frame rate follows
the focused Niri display's refresh rate, capped by `max_fps`; an explicit `fps`
overrides that cap. With portal capture the selected screen may differ from the
focused display; choose an explicit rate when that distinction matters.

The portal asks which screen/window to share, and can remember approved sessions.
`capture = "focused-output"` uses Niri's current output directly, with the
permissions required by GPU Screen Recorder's native monitor backend. A discovered
output name can also be configured. `restore_portal = false` asks for selection
on each recording.

Hardware encoding is required: Foundation disables CPU-encoding fallback so a
missing GPU capability cannot unexpectedly consume the CPU. `video_codec = "auto"`
uses the recorder's hardware default; explicit codecs fail if unsupported.
Container, video quality, frame mode, cursor, output directory, capture source,
audio source roles, codec/bitrate, shortcuts and bounded startup/stop waits are
configurable. Shortcut changes take effect on configuration redeployment;
other settings are read for the next recording.

Audio defaults follow `default_output` and `default_input`, preserving the current
system-selected playback and recording sources, including processing selected by
the user. Separate `-a` options create independent tracks. No volume, sample-rate,
EQ or EasyEffects setting is changed. Opus defaults to 512 kbps **per track**;
it is high-bitrate lossy audio, not lossless. GPU Screen Recorder negotiates the
source formats. There is no extra lossless capture or muxing pipeline.

Files go to XDG Videos/Recordings unless `directory` is an absolute override.
MKV is the default so an interrupted recording can remain recoverable. Names use
unique timestamps; existing recordings are never selected for overwriting.

## Lifecycle

One kernel datagram socket activates a single user-service owner. While recording,
the owner waits for requests, child-exit signals and first-frame filesystem events;
it does not periodically poll. The recorder and owner exit after saving. The
portal selection can be cancelled before capture begins; a bounded grace period
handles selectors that do not respond to the recorder's normal stop signal.
Once a frame is reported, the normal save/finalization window applies instead.
The Quickshell clock watches the atomically published runtime JSON without a polling
timer. No service is enabled to record at login.

Source references: [GPU Screen Recorder](https://git.dec05eba.com/gpu-screen-recorder/about/),
[upstream packaged CLI manual](https://man.archlinux.org/man/extra/gpu-screen-recorder/gpu-screen-recorder.1.en),
[Niri screencasting](https://github.com/niri-wm/niri/blob/main/docs/wiki/Screencasting.md).
