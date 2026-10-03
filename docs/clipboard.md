# Clipboard history

Super+V opens an on-demand surface built from the launcher's approved graphite
palette, typography, card, search input and keycaps. Arrow keys select, Enter
copies and closes, Escape dismisses. Click a row to copy, its × to delete, or
Ctrl+Delete to remove the selected item. Clear all requires inline confirmation;
Escape cancels it. Search matches the first 2048 characters of text and the MIME
label. Text is displayed as plain text, never rich text. Image thumbnails are
bounded to 72 pixels when decoded for display.

Two native wl-paste watchers subscribe to text and image clipboard events. Each
change invokes the shared, short-lived Rust backend through `scripts/foundation
clipboard`; there is no polling or resident storage daemon. SQLite deduplicates by MIME plus exact bytes and retains
up to 100 items, 8 MiB per item and 32 MiB of retained payload. UTF-8 text and
PNG/JPEG/GIF/WebP image bytes are copied unchanged. Unsupported formats are skipped.

History lives in $XDG_STATE_HOME/desktop-foundation/clipboard (normally
~/.local/state/desktop-foundation/clipboard). The directory is private (0700),
files are 0600. History is persisted without encryption. Native sensitive,
empty and cleared clipboard events are skipped. Sensitive exclusion depends on
the source application supplying the hint; unmarked passwords can enter history.
Clear removes history and image previews, but leaves the current clipboard
selection alone. SQLite secure_delete is enabled; this is not secure erasure of
filesystem snapshots or backups.

The text/image watcher definitions are persistently deployed and started by
session-start through desktop-foundation-session.target after history
initialization. They are tied to the foundation and graphical session targets,
restart on failure with a bounded burst, and stop when the session ends. Shell
crashes do not stop collection. The QML surface and storage worker use Wayland
clipboard tools; Niri is the active reference compositor.

Current storage validation includes Rust integration tests and CLI regression tests. Historical interactive
validation: the finite
live smoke test checks a real mouse click on an existing item in the deployed
config before exercising isolated history. It preserves the current clipboard's primary
format and sensitive hint, and restores the real watchers. It verifies Super+V,
sensitive exclusion, deduplication, exact Finnish Unicode/newline and PNG copies,
search, deletion, clear confirmation/cancellation and destruction after dismissal.
The demonstration screenshot contains controlled history fixtures. Restoring a
single primary MIME does not reconstruct all formats originally offered by an app.

# XWayland popup artifact

Brave Origin Nightly and ChatGPT were both XWayland clients. The demonstrated
ChatGPT menu had a rectangular blurred shadow margin. Disabling compositor
shadows did not remove it; disabling blur did. A persistent no_blur rule for
floating XWayland surfaces removes that rectangle while preserving shell-layer
and tiled-window blur. The change was verified on the open ChatGPT menu across
config reload, without restarting either application. It also means ordinary
floating XWayland windows do not receive compositor blur. Actual popup coordinate
misplacement in other cases has not been established by this test.

A five-second idle sample after closing the surface measured both watcher processes
at 0% CPU ticks, roughly 2.2 MiB RSS each and 0.2 MiB PSS each. The warm resident
shell measured 218 MiB RSS / 167 MiB PSS and 0% CPU ticks in that interval. These
are finite observations, not guarantees of zero wakeups or a clipboard-only
increment; the shell also contains the launcher and compositor adapter.

The clipboard worker uses Quickshell.shellDir plus a filesystem-relative path,
like the launcher executable. Qt.resolvedUrl must not resolve executables outside
the config tree: Quickshell redirects these URLs to qrc:/qs-blackhole. The isolated
harness overrides DF_CLIPBOARD_WORKER explicitly, and the deployed click check
covers the default path so that override cannot hide a production path failure.


## History projection cost

History export reads item metadata and at most 8192 bytes per text preview.
Existing image sidecars avoid rereading every stored image BLOB; missing sidecars
are rebuilt from the exact original payload. UTF-8 previews remain bounded to
2048 characters, including embedded NULs and multibyte characters at the boundary.
Copy is a read-only history action: it passes the exact selected payload to
wl-copy without replacing the watched index or forcing the open list to reload.
Store, delete, clear and initialization still publish the index atomically under
the existing database lock. Storage limits and private file permissions remain.

The Rust worker uses the existing SQLite schema, MIME-plus-NUL-plus-payload SHA-256
IDs and index fields. Migration requires no history conversion. Temporary files
are unique and atomically renamed, paths from the database are validated, and
corrupt history is reported without resetting it. Cleanup removes only recognized
history projections. Build with `scripts/build-backend`; normal installation builds
the backend before deploying units. `scripts/clipboard.py` is an exec-only
compatibility entry point for older generated units, and contains no storage logic.
