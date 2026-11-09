# Replay editor reference-host validation

2026-11-09, Lucky38: NVIDIA RTX 3060, driver 620.70.06, GPU Screen Recorder
6.1.7, FFmpeg 9.0.2, Niri 26.04. These are observations on this host, not a
cross-hardware certification or a claim that fixture tests prove physical audio.

The new recorder owner ran in a private validation session using the actual
DP-1 display and existing default desktop/microphone routes. No live desktop
configuration, device volume, EQ, audio routing or original recordings were
changed by the experiment.

| Check | Observed result |
| --- | --- |
| Native capture | 2560 × 1440, 60 FPS, H.264 on NVENC, two separate stereo Opus tracks |
| Continuous buffer | Configured 300 seconds; same GPU Screen Recorder process throughout snapshots, ordinary recording, editing and quick-save |
| Early editor | Actual 2.107-second source, ten generated thumbnails; prepared in 2.21 seconds while ordinary recording shared the recorder |
| Full editor | Actual 301.067-second protected snapshot; keyframe padding explains the extra history; prepared in 3.27 seconds |
| Selected clip | Last 47 seconds exported to a validated 47.001-second MKV, native dimensions, two Opus tracks; finite export took 16.21 seconds |
| One-key path | Full replay save produced a 300.550-second MKV without restarting or clearing the recorder |
| Capture continuity | Final replay had 18,033 video packets; maximum adjacent presentation-time gap 20 ms, no gaps exceeding 50 ms |
| RAM | Recorder RSS approximately 1.03 GiB with a full buffer; not a negligible-memory feature. Encoded history plus driver/encoder overhead accounts for the total. |
| Shutdown | Owner exited successfully and published replay Off; only its own recorder was stopped |

Additional real-media tests used independently timed stereo/mono audio markers
and moving video: a 3.5-second NVENC cut produced 210 frames at 60 FPS, preserved
both channel counts and Opus tracks, and retained marker timing within 40 ms.
The whole-snapshot export matched the source bytes exactly. Ten JPEG thumbnails
were decoded and compared; a scrub request produced a real decoded preview.

The production QML editor also loaded against real media in an offscreen host.
Its range handles, precise time inputs and keyboard scrubber use the shared
components. Offscreen fixtures do not establish physical focus behavior or audio
latency. Live artifacts remain private on the reference host; screen recordings
and microphone data are not committed to the repository.
