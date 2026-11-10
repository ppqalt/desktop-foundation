# Desktop completion inventory

Historical baseline inspection: 2026-11-08, isolated branch based on `28cd074`. This is an
engineering backlog, not a release checklist marked complete by documentation.
The active Lucky38 owner was `c549bf3`; its deployment manifest points to the
owner checkout, not this worktree. Its local untracked font backup and the
uncommitted Transparency work are preserved. The live Niri package was stock
26.04 and the Foundation shell and wallpaper services were active at inspection.
Reinspect host state before any later deployment.

| Area | Evidence and present state | Completion boundary |
| --- | --- | --- |
| Tokens and controls | `theme/Theme.qml` and `Material.qml` are split authorities; shared action, choice, switch, slider and scrollbar exist. Gallery covers a subset. | Establish one coherent token contract, migrate real consumers, cover all implemented variants, contrast, text scaling, light/dark palettes and keyboard/accessibility. Do not mistake an offscreen render for accessibility acceptance. |
| Interfaces | [Design-system inventory](design-system-inventory.md) lists actual surfaces and provider boundaries. Control Center already shares controls; launcher, clipboard, Bluetooth, power and wallpapers retain domain-specific interaction. Mako and Kitty/btop render outside QML. | Audit and migrate each interface incrementally without replacing providers or regressing routing, focus or compact layouts; measure latency and memory rather than assume improvements. |
| Motion | Current shared tokens use duration and Bezier approximations, including reduced motion. [Google's motion physics guidance](https://m3.material.io/styles/motion) uses spring-based schemes. | Evaluate interruptible springs on representative surfaces with reduced-motion and performance tests; preserve established compact desktop motion rather than applying bounce globally. |
| Transparency | Uncommitted transaction changes attempt to retain application identity across stream recreation; legacy journals lacked identity. Initial fixture tests exercise both. Restoration after a stream is absent while mode turns off is not implemented. | Complete persistent, unambiguous restoration across off/restart/stream recreation; retain external volume changes and monitoring independence. Validate isolated PipeWire fixtures and hardware separately; do not deploy partial changes. |
| Recording | The default is now 300 seconds with configurable automatic startup, MKV and Opus. The editor pins a non-clearing snapshot from the same recorder; shared range controls select the exported portion. | Real-media thumbnail/full-copy/NVENC tests cover exact cuts, channel counts and timed audio markers. Complete protected live acceptance and resource measurements before deployment; no-audio capture remains separate work. |
| Wallpapers | Image inventory and lazy previews support many formats, with optional FFmpeg decoding for JXL/HEIC. Picker has four columns. | Video playback, animated previews, per-output assignments, audio, fullscreen suspension, hotplug and transactional recovery need investigation and actual runtime proof. Do not infer playback from a probe. |
| Niri | A patched package and signed stock rollback archive were prepared separately; the live machine still uses stock Niri. Ctrl+wheel behavior was not physically verified. | Complete package ownership, automatic fresh-install and updater integration, rollback, protected session transition, fresh login and physical input checks. Never claim package preparation is activation. |
| Nautilus | `application_roles.rs` contains XDG default handling; wallpaper theme propagates a supported GNOME accent. Existing user GTK4 CSS is outside Foundation's managed targets. | Preserve MIME preferences and user's CSS; establish a safe, maintainable color integration and test actual file opening/theme behavior without globally breaking GTK applications. |
| Installer and updater | Minimal, standard and explicit extras exist; isolated-branch bootstrap and recording-extra fixes are committed. The patched Niri package is not wired into the normal installation/update transaction. | Test clean install, ownership conflict, optional selections, interrupted upgrade, exact versions and rollback in isolated hosts, then verify protected live paths only with authentication. |
| Settings, notifications, search and diagnostics | Control Center and launcher exist; notifications are provided by Mako, with no Foundation-owned QML history center established by this inventory. A broad validated TOML settings editor is not established. | Examine existing CLI/config owners before implementing useful incremental integrations. Provide bounded actions, validation, recovery and accessibility; do not add a parallel settings database or persistent process by default. |
| Acceptance | Component and isolated Rust/QML fixtures exist. Current live desktop has not received these branch changes. | Verify hardware, compositor focus, displays, accessibility, fresh login, rollback and real capture separately. Publish neither a release nor a complete-success claim until acceptance evidence exists. |

Protect the single owning deployment manifest, user configuration and recovery
journals. Keep incomplete audio changes out of live deployment. The user has
authorized administrator authentication when a reviewed protected operation is
ready; never request or handle their password in chat.

## Source reconciliation and follow-up, 2026-11-10

The owning Lucky38 checkout now contains the validated UI changes through
`1968d5c`, including the sidebar section/Audio navigation fixes, update Check
action, replay centisecond fields and thumbnail focus frame, btop outline and
nonintrusive reload notices. Local `df-update` option 0 now selects these commits;
it does not search other worktrees. No updater installation was run as part of
this source handoff.

Transparency restoration now retains application identity/channel maps, safely
rebinds recreated streams, keeps absent application histories after Off and
reloads that journal after restart. Monitoring restoration is completed once;
subsequent playback recovery does not require EasyEffects. An event-driven owner
waits only while restoration is outstanding. Finite login recovery does nothing
when no journal exists. Tests cover inherited/manual levels, legacy journals,
ambiguous identities, restoration after Off and no-journal startup. Physical
audibility, the device route and real service/session acceptance remain separate
checks; fixture success does not establish them.

The older table remains a backlog rather than a claim that video wallpapers,
complete settings/notification interfaces, all gallery variants or patched Niri
activation have been finished. Those require further implementation and their
own acceptance evidence. Preserve the desktop and source ownership while doing so.

Gallery follow-up: independent shared typography preview at 100/125/150/200%
now complements composition zoom. Action/dropdown geometry adapts to larger line
heights; gallery descriptions wrap and layout minimums allow narrow widths.
Production-control tests verify the larger two-line/compact action heights,
scale clamping and return to default geometry. This is a process-local gallery
preview, not system-wide text scaling or completed accessibility acceptance.

Component parity follow-up: gallery and Control Center now share `FocusViewport`
for scaled/resized focus reveal and manual-scroll precedence. Search and replay
time inputs share Tab focus, disabled treatment and text-size-aware field height.
The gallery's disabled-input mode includes dropdown and search, and its replay
selection caption now matches the production centisecond precision. Focus/input
fixtures cover these shared implementations; this does not establish full parity
for every remaining specialized desktop surface or physical accessibility tools.

Popup/timeline/focus follow-up: the shared PopupCard now fills a background parent
when no explicit size is supplied, restoring its surface and outline in the
gallery. Replay selection frames use actual handle centers, including mirrored
scrubbing and keyboard direction. Control focus rings distinguish keyboard
navigation from clicks without removing state feedback or popup borders. These
paths have focused production interaction/real-media tests; wider component and
physical accessibility acceptance remains open.
