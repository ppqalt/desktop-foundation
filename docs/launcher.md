# Launcher and surface language

Super+Space opens/closes the launcher on the focused output. Type an application
name, generic name or keyword. Exact/prefix names rank first; fuzzy name matching
handles abbreviations. Secondary descriptions require a real substring to avoid
irrelevant scattered-letter matches. Hidden or commandless desktop entries are
excluded. Desktop-entry additions/removals come from Quickshell's native watched model.
In-place metadata edits did not refresh reliably in the installed 0.3.1 build;
restart the shell after manually changing an existing .desktop file. No polling
workaround is installed.

Up/down (also Ctrl+N/P), Tab/Shift+Tab navigate; Enter launches; Escape or an
outside click closes. Click a result to launch directly. Empty results have an
explicit state and Enter does nothing. No arbitrary command execution, calculator,
clipboard contents, network search or permanent bar is included. Search text is
never evaluated by a shell. Entry commands use parsed argument arrays, working
directory and explicit Kitty wrapping for Terminal=true, then the existing UWSM-
aware launch helper. This avoids the installed DesktopEntry.execute() limitation
that ignores Terminal=true. No launcher subprocesses run while searching.

The launcher and Applications object are lazily created/destroyed. The native
DesktopEntries singleton and Qt icon/font/GPU caches may remain resident after
first use; destroying a surface does not promise all cache memory returns.

## Visual foundation

A dark graphite glass card with restrained blue-grey accent, thin borders, 24 px
outer radius, 12 px selection radius, 14 px body text, 22 px search text and compact
monospaced keycaps. Adwaita Sans/Mono are installed; Qt's normal fallback applies
if unavailable elsewhere. Six result rows at most, adapting down to a compact
minimum height. 160 ms ease-out entrance/resize and 120 ms exit. No permanent
chrome. Scoped Hyprland layer blur ignores the scrim alpha; QML owns motion to
avoid combining independent compositor and QML layer transitions.

Reusable components: SurfaceCard (glass/shadow), SearchInput, Keycap; Theme owns
semantic colors, typography, spacing, radii and motion. ApplicationRow stays app-
specific. Future clipboard work will use these primitives only after user review.
The current direction is an implemented visual candidate awaiting that review.
No clipboard service, history storage or clipboard UI has been started.

## Verification

`scripts/check` and seven tests pass. JS tests cover ranking, hidden/invalid entry
filtering, multi-token search, fuzzy names and literal shell-like input.
`python3 scripts/launcher-smoke.py` uses a temporary desktop entry and uinput device:
real Finnish Super+Space, actual text input, arrows, Escape, Enter launching owned
Kitty (including a Terminal=true entry), empty results, outside mouse click and lazy teardown. Fixtures are removed
and original workspace restored. `/dev/uinput` access is needed for that optional
live test; it does not install or enable a virtual keyboard service.

`python3 scripts/launcher-bench.py` measures the warmed resident shell hidden,
open with a blinking caret and destroyed, plus object creation/destruction IPC
roundtrips. Samples are finite; they do not establish frame presentation latency
or zero future wakeups. Open caret/rendering activity is expected. After dismissal
there is no visible layer and no recurring launcher work.

Upstream API reference:
[DesktopEntry](https://quickshell.org/docs/v0.3.1/types/Quickshell/DesktopEntry/).
