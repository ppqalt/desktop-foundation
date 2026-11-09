# Recording

Install the optional `gpu-screen-recorder` package, or select `--extra recording`
in the installer. Foundation owns recording through its Rust backend; GPU Screen
Recorder handles capture, encoding and instant replay in one process.

- **Ctrl+Shift+F12:** start or stop recording with separate desktop and microphone tracks.
- **Ctrl+Shift+Alt+F12:** record desktop audio only, with no microphone track.
- **Ctrl+Shift+Alt+F11:** start or stop the instant replay buffer.
- **Ctrl+Shift+F11:** save the buffered replay. If replay is off, this starts the
  buffer and saves whatever is available after the first frames arrive.

Replay defaults to a continuous **five-minute (300-second) RAM buffer** with
separate desktop and microphone tracks. With the recording extra installed,
`desktop-foundation-replay-start.service` requests replay from the same socket
owner at session startup. Set `replay_auto_start = false` in the user recording
TOML to disable automatic startup; the existing toggle still stops or starts it
for the current session. Existing explicit duration overrides remain effective.
The portal may require screen-sharing approval before capture can start.

A 16 Mbps video limit plus two 512 kbps audio tracks is approximately 638 MB of
encoded five-minute history, plus encoder/recorder overhead. Actual RAM use is
higher and depends on the GPU and source: the reference-host run measured about
1.03 GiB for the recorder with a full buffer. See the
[validation report](research/replay-editor-validation.md). Early saves contain only available
history; the first video keyframe may precede the nominal buffer boundary.
During replay, ordinary recording uses the same capture process. Desktop-only
recording pauses and clears replay, then restores buffering after it stops.
There is never a second continuous recorder for the editor or thumbnails.

## Save Replay editor

The Control Center's **Save** action opens a compact editor. It first asks the
existing recorder for a snapshot using `save-replay` with `restart-replay=false`.
The snapshot is pinned into a private session before being exposed to the editor.
The RAM buffer and any simultaneous ordinary recording continue unchanged.
**Ctrl+Shift+F11 remains a one-key save of the full available buffer.** Neither
save path clears the rolling buffer.

The timeline shows real frames extracted from that immutable snapshot. Drag the
range handles or edit Start/End as `minutes:seconds.milliseconds`; the separate
scrubber requests an actual frame preview. The selected duration is displayed.
The range selects **which footage to export**, never how long the rolling buffer
is. The editor initially selects up to the last 47 seconds.

The editor has one trim range control, shared with the component gallery.
Click or drag the thumbnail strip to scrub the preview without changing the
selected clip. Tab to the strip and use Left/Right for frame steps; the two
trim handles retain the shared slider keyboard and focus behavior.

Thumbnail and scrub jobs run asynchronously outside QML, one job per snapshot.
Ten small timeline frames are generated with bounded single-thread FFmpeg jobs.
Scrub requests coalesce while dragging and cache at most 128 larger preview
images. There is no thumbnail daemon or additional capture pipeline.

Saving the whole snapshot copies its MKV packets without re-encoding. Arbitrary
precise cuts use NVENC rather than silently snapping the start to an earlier
keyframe. Each Opus track is independently trimmed and encoded, with its original
channel count; bitrate is capped to libopus's supported per-channel limit.
The output retains native dimensions. Export validates MKV, video/audio packets,
two Opus tracks, duration, dimensions and track start-time alignment before
publishing a collision-resistant `Clip-LOCAL_TIMESTAMP.mkv`. A failed encode or
validation leaves an error and retains the source and any partial output.
See [FFmpeg accurate seeking](https://www.ffmpeg.org/ffmpeg.html): exact arbitrary
stream-copy boundaries cannot be assumed.

**Save Replay** captures the current buffer and opens the editor directly. Trim
and choose **Save clip**; validated export removes its temporary editing source.
There is no draft browser or second capture button in this normal flow.
Opening Save Replay again prepares fresh footage, then replaces the previous
interactive workspace only after the new source and thumbnails are valid.
Failed preparation or export preserves footage for recovery. Existing legacy
recovery drafts are untouched and do not block the interactive editor.

Temporary sources live under
`$XDG_STATE_HOME/desktop-foundation/replay-editor/`. Each source is limited to
2 GiB. Three consecutive incomplete interactive workspaces stop preparation
rather than allowing failed captures to consume unbounded disk space. Recovery
commands `editor list`, `resume ID` and `discard ID` remain available; discarding
never deletes exported recordings. Saved and partial exports are not overwritten.

Finite diagnostic commands use the same backend:

```sh
scripts/record editor list
scripts/record editor open
scripts/record editor resume SESSION_ID
scripts/record editor preview SESSION_ID 253.250
scripts/record editor save SESSION_ID 253.000 300.000
scripts/record editor discard SESSION_ID
```

The clock appears automatically during capture. Its red indicator starts only
after GPU Screen Recorder reports its first captured frame. It remains visible
while stopping and while checking the completed container. It clears only after
the recorder exits successfully and `ffprobe` confirms a nonempty video with the
expected audio tracks and readable duration. This checks container finalization,
not a full decode of every frame.
A small outlined indicator distinguishes an active replay buffer; the red
recording indicator remains visible during a simultaneous recording. The normal
clock shortcut can still hide or show it while recording, without stopping capture. If left visible, the indicator stays through finalization.

On failure, a warning remains in the clock and a notification explains the error.
Partial files are retained. Inspect `scripts/record status`; use
`scripts/record acknowledge` to dismiss the warning after inspection.

## Settings

Defaults: [config/recording.toml](../config/recording.toml).
Override selected values in `$XDG_CONFIG_HOME/desktop-foundation/recording.toml`
(normally `~/.config/desktop-foundation/recording.toml`). Sorted fragments in
`recording.d/*.toml` load before that file. `scripts/record plan` prints effective
settings, sources, the focused display and the exact proposed command without
starting capture. `scripts/record plan --silent` shows the desktop-audio-only variant.

```toml
resolution = "auto"
fps = 0
max_fps = 60
audio_bitrate_kbps = 512
```

`auto` resolution passes through the actual captured source's dimensions. Choose
a display in the portal for native display resolution. Scaling overrides are rejected.
No monitor name, GPU model or rendering device is fixed. Automatic frame rate follows
the focused Niri display's refresh rate, capped by `max_fps`; an explicit `fps`
overrides that cap. With portal capture the selected screen may differ from the
focused display; choose an explicit rate when that distinction matters.

The portal asks which screen/window to share, and can remember approved sessions.
On Niri, a globally forced `GDK_BACKEND` can prevent the GNOME portal from
exposing screencasting. If `scripts/record status` reports a portal failure,
check the portal logs and ensure that variable is unset for
`xdg-desktop-portal-gnome.service`; restart the portal services only when no
screen-sharing session is active. Other applications can retain their own backend
settings. See the [Niri portal guidance](https://github.com/niri-wm/niri/wiki/Important-Software#portals).
`capture = "focused-output"` uses Niri's current output directly, with the
permissions required by GPU Screen Recorder's native monitor backend. A discovered
output name can also be configured. `restore_portal = false` asks for selection
on each recording.

Hardware encoding is required: Foundation disables CPU-encoding fallback so a
missing GPU capability cannot unexpectedly consume the CPU. `video_codec = "auto"`
uses the recorder's hardware default; explicit codecs fail if unsupported.
MKV and Opus are required. Video quality, frame mode, cursor, output directory,
capture source, audio source roles, bitrate, shortcuts and bounded startup/stop
waits are configurable. Shortcut changes take effect on configuration redeployment;
other settings are read for the next recording.

Audio defaults follow `default_output` and `default_input`, preserving the current
system-selected playback and recording sources, including processing selected by
the user. Separate `-a` options create independent tracks. Desktop-only recording
uses only the output source. No volume, sample-rate,
EQ or EasyEffects setting is changed. Opus defaults to 512 kbps **per track**;
it is high-bitrate lossy audio, not lossless. GPU Screen Recorder negotiates the
source formats. There is no extra lossless capture or muxing pipeline.

Files go to XDG Videos/Recordings unless `directory` is an absolute override.
MKV is the default so an interrupted recording can remain recoverable. Names use
readable local timestamps and unique suffixes; existing recordings are never
overwritten. Desktop-only files start with `Desktop-`, normal recordings with
`Recording-` and replays with `Replay-`.

## Lifecycle

One kernel datagram socket activates a single user-service owner. While recording,
the owner waits for requests, child-exit signals and first-frame filesystem events.
Replay uses the same owner and recorder; it waits on recorder events and requests.
The recorder and owner exit when recording or replay stops. The
portal selection can be cancelled before capture begins; a bounded grace period
handles selectors that do not respond to the recorder's normal stop signal.
Once a frame is reported, the normal save/finalization window applies instead.
If capture format changes while replay is active, replay stops safely; any active
normal recording is finalized first. Restart replay to capture the new mode. A
recorder crash leaves partial files in its private staging directory for recovery.
The Quickshell clock watches the atomically published runtime JSON without a polling
timer. Automatic replay startup uses the same recorder owner and respects `replay_auto_start`.

Source references: [GPU Screen Recorder](https://git.dec05eba.com/gpu-screen-recorder/about/),
[upstream packaged CLI manual](https://man.archlinux.org/man/extra/gpu-screen-recorder/gpu-screen-recorder.1.en),
[Niri screencasting](https://github.com/niri-wm/niri/blob/main/docs/wiki/Screencasting.md).


## Verification

`cargo test --test recording` checks lifecycle, early saves, desktop-only
interruptions, non-clearing editor snapshots, continued ordinary recording and
clean shutdown using an inert recorder. `cargo test --test replay_editor` uses
real encoded media for thumbnail images, scrubbing, full-copy integrity,
validation, collision resistance and invalid-input protection. Run
`DF_TEST_NVENC=1 cargo test --test replay_editor` on the NVIDIA reference host to
require the exact-cut test, including video frame count, stereo/mono channel
preservation and independently timed audio markers. The NVENC test is explicitly
hardware-dependent; portable tests do not claim it ran.

`tests/test-replay-editor` loads the production editor and real thumbnails in an
offscreen Quickshell host, including range edits and keyboard scrubbing. It
requires a built backend, FFmpeg and Quickshell. These finite tests do not by
themselves prove a live five-minute capture or physical audio routing.
