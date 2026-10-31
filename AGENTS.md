# Desktop Foundation engineering handbook

## 1. Read this first

**Before changing any window, widget, menu, popup, overlay, shared control, input
handler, theme token or UI/backend contract, read
[docs/FOUNDATION_UI_STANDARD.md](docs/FOUNDATION_UI_STANDARD.md).** It is the
normative detailed UI engineering standard, not optional inspiration.
Also consult the [official Google Material reference index](docs/MATERIAL_REFERENCES.md)
for the relevant foundation, style, component specification and accessibility page.

This root file is the primary entry point for coding agents. It documents the
repository's architecture, mandatory constraints, workflows and known gaps.
It is not a claim that every existing file already satisfies every requirement.
Verify the current checkout and affected callers before relying on an older note.

### Every widget belongs to one system

All existing and future widgets MUST follow the category contracts in the
[Widget Standard](docs/FOUNDATION_UI_STANDARD.md#widget-standard). This includes
popups, Control Center modules, sidebars, readouts, notifications and persistent
surfaces—not just newly created widgets. **Unify the rules, not the functionality.**
The same interaction must have the same result within a category: shortcut
toggles, interrupted transitions, focus, Escape, directional navigation, loading,
confirmed state, errors and cleanup must not depend on which widget was copied.

Before refactoring, compare actual implementations in the
[architecture audit](docs/architecture/widget-audit.md). Reuse the existing
router, lifecycle, focus, surface, motion and request primitives. Do not create
copied controllers that merely behave alike: overlapping requirements MUST run
through the same shared implementation. Centered popups use `PopupWindow`,
`PopupCard` and `PopupLifecycle`; finite backend requests use `ControlRequest`.
Keep domain operations outside these foundations. Do not create
one giant conditional widget, parallel lifecycle protocols, or a Rust worker for
pure presentation. Preserve explicit category exceptions and test representative
consumers, including rapid handoffs and async results after close. Document gaps
as gaps; updating the standard alone does not make old implementations compliant.
The left Control Center and right Kitty/btop monitor belong to the same sidebar
category. Their renderer/save adapters differ; do not exempt btop from shared
geometry, motion, focus and accessibility infrastructure. See the
[mirrored sidebar comparison](docs/architecture/widget-audit.md#mirrored-sidebar-comparison-2026-10-31).
All interactive widget activation, including Super+X, enters `SurfaceRouter`.
External renderers use its `external` adapter and `ExternalSurface` transport;
do not bypass mutual dismissal by starting the monitor directly from a shortcut.

### Rule vocabulary and precedence

- **MUST / MUST NOT:** mandatory project requirements.
- **Prefer / SHOULD:** the default convention; depart only with a concrete reason.
- **Consider:** context-dependent advice, not an instruction to expand scope.
- **Current limitation:** observed implementation, not intended architecture.

Explicit user instructions take precedence. Then follow this handbook and the
specialized UI standard. Use actual source to establish what currently works;
use these requirements to establish what a new change must preserve or improve.
If source and documentation disagree, identify the discrepancy rather than
silently asserting that the documented feature exists.

## 2. Purpose and engineering priorities

Desktop Foundation is a repository-managed, barless desktop environment built
around Niri, Quickshell and native Rust helpers. It combines configuration,
interactive surfaces, wallpaper-driven theming and reversible installation.
It is not a replacement operating system, a general package manager or a browser
profile manager.

Prioritize reliability, immediate input response, consistency, maintainability
and visual quality. A functional prototype is not a polished interaction:
loading, keyboard access, failure handling, repeated shortcuts, shutdown and
restoration are part of implementing the feature, not optional finishing work.

MUST preserve working functionality before adding new features. During a
standardization or regression task, do not introduce unrelated user-facing
features. Prefer improving an existing shared primitive over creating another
slightly different implementation. A new abstraction must have an actual
responsibility and callers; do not create an unused alternative component library.

Keep the desktop minimal: remove redundant labels, counts, decorative diagnostics
and permanent instruction legends. Preserve meaningful status, failures, pending
operations and confirmations. Minimal wording is not permission to hide errors.

## 3. Supported environment and compatibility policy

**CachyOS + Niri is the only officially supported reference environment.** Other
distributions/compositors are unsupported, not prohibited. Do not promise their
compatibility, add additional official integrations without authorization, or
introduce arbitrary distribution-name checks that reject an otherwise capable
machine. Check actual binaries, protocols, configuration features, services and
package-manager requirements. A missing Niri capability is a legitimate failure;
a different `ID` in `/etc/os-release` alone should not be.

The current live shell and normal deployment route select Niri. Archived
`compositor/hyprland/` and shell adapter sources are not an actively maintained
second installation path. Preserve recovery of older deployment journals.

**Current gap:** earlier code still contains explicit platform/compositor gates:
`native/foundation/src/platform.rs`, `deploy.rs`, `preflight.rs`, `update.rs` and
`scripts/bootstrap`. The capability-based policy above supersedes the older
“reject every other platform” instruction. These existing gates have not been
removed by this documentation task. Do not describe that migration as complete,
or bypass safety checks indiscriminately to make another platform appear supported.

## 4. Mandatory architectural constraints

- MUST use **Rust for backend behavior, tools and system integration**, and
  **QML/Quickshell for presentation and immediate UI interaction**.
- MUST NOT add Python implementation, tests, installer logic, automation or new
  Python runtime dependencies. The target is no Python anywhere in these paths.
  Existing legacy Python remains a migration gap, not a precedent to extend it.
- MUST reuse shared components/tokens and follow official Google Material 3
  Expressive guidance. Do not invent measurements or claim an approximation is
  an official token. Record desktop-specific adaptations in the UI standard.
- MUST NOT add shadows: this includes cards, popup effects, Niri windows and
  sidebars. Tonal surfaces, spacing and focus outlines express hierarchy.
- MUST preserve wallpaper-derived semantic colors, Google Sans Flex UI type,
  appropriate monospace exceptions, existing shortcut ownership and the barless UI.
- MUST preserve user settings, original backups, ownership journals and unrelated
  working-tree edits. Never turn a UI cleanup into an account-data migration.
- MUST NOT push, tag, publish, release or send messages externally without a request.

## 5. Repository map and documentation hierarchy

Paths below are relative to this repository root. Read the implementation as well
as its documentation when changing behavior.

| Location | Responsibility / entry point |
| --- | --- |
| `native/foundation/` | `desktop-foundationctl` crate, backend modules, integration tests, scoped helper binaries |
| `native/nothing/` | Separate Tokio/BlueZ RFCOMM Nothing/CMF protocol helper |
| `shell/shell.qml` | Composition root, lazy loaders, surface routing, shell IPC |
| `shell/surfaces/` | Centered popups, control center, readouts and backdrop windows |
| `shell/components/` | Shared controls, rows, focus, wheel and motion primitives |
| `shell/services/` | Application catalogue and clipboard projection/process bridge |
| `shell/adapters/niri/` | Niri event/state normalization and compositor operations |
| `shell/theme/` | `Theme.qml`, `Material.qml`, fonts/symbol registration and tokens |
| `shell/utils/Search.js` | Presentation-side search ranking; not a backend service |
| `config/` | Shared input/window intent, roles, sidebar, local-AI and app-mod settings |
| `compositor/niri/` | Native bindings and base Niri configuration |
| `theme/templates/niri.kdl.in` | Template consumed by the native Niri renderer |
| `theme/fallback/` | Static fallback semantic/Material/application palettes |
| `theme/adapters.json` | Adapter refresh-mode descriptions; verify native implementation when changing |
| `profiles/` | Default hardware intent and example output overrides |
| `packages/` | Installer package manifests; not updater package actions |
| `scripts/` | Public command wrappers plus remaining legacy provisioning helpers |
| `session/` | Greetd/PAM/keyring and other session integration assets |
| `terminal/` | Kitty, Fish, Fastfetch and monitor shader configuration |
| `audio/` | WirePlumber Bluetooth configuration |
| `apps/` | Personal applications and pinned modification/provisioning sources |
| `browser/brave/` | Managed browser assets/legacy configuration; not permission to edit a live profile |
| `fonts/` | Repository-managed font assets |
| `tests/` | Offscreen QML runner, fixtures and remaining legacy tests |
| `.github/workflows/check.yml` | Portable CI; does not certify a live desktop |
| `docs/` | Specialized engineering and user references |

Documentation hierarchy:

1. This `AGENTS.md`: project-wide requirements and engineering workflows.
2. `docs/FOUNDATION_UI_STANDARD.md`: detailed normative UI contract and component APIs.
3. `docs/MATERIAL_REFERENCES.md`: official Google specification discovery index.
4. Task-specific docs: `docs/backend.md`, `docs/clipboard.md`,
   `docs/control-center.md`, `docs/monitor.md`, `docs/nothing-controls.md`,
   `docs/bluetooth.md`, `theme/README.md`, `docs/deployment.md`,
   `docs/updating.md`, `docs/recovery.md`, `docs/keyring.md`,
   `docs/browser-policy.md`, `docs/app-modifications.md`,
   `docs/rust-migration.md` and `docs/diagnostics.md`.
5. `docs/research/`: background/inspiration, not authority over current Google
   specifications or explicit project requirements.

No repository-owned `SKILL.md` package or second agent instruction file was found
in this revision. Environment-provided skills are separate from project source;
consult one when it applies, but do not assume private assistant memory or an
external skill is available to the next contributor. Put lasting project rules
here and link specialized references rather than duplicating their contents.

## 6. Rust backend and process contracts

### Crates, binaries and public commands

`native/foundation/Cargo.toml` defines the `desktop-foundationctl` package using
Rust 2024. `src/main.rs` parses commands and dispatches to modules exported by
`src/lib.rs`. Supporting binaries are `foundation-oo7-ready` and
`foundation-browser-policy`; `foundation-measure` is gated by the `bench` feature.
Dependencies include serde/JSON/TOML, system SQLite through rusqlite, image,
SHA-256 and Wayland client/protocol crates. Do not add a framework merely to
wrap an existing native command.

`scripts/foundation` resolves the checkout's release executable and supplies
`--root`. It does not silently compile a missing backend. `scripts/build-backend`
builds the main executable and scoped helpers with Cargo's lockfile; it currently
pins toolchain `1.98.1`, while `rust-toolchain.toml` selects stable developer tools.
Check both before changing toolchain policy. Build Nothing separately with
`scripts/build-nothing`. Keep callers and release binaries from the same source.

`native/nothing/src/main.rs`, `protocol.rs`, `settings.rs` and `input.rs` own the
earbud protocol. Its line-oriented JSON commands/events and bounded transport
are separate from PipeWire playback codec selection in the Foundation backend.
Do not reintroduce earbud firmware-quality changes where the UI requests a host
PipeWire SBC/SBC-XQ/AAC/LDAC playback change.

### Source ownership map

| Module under `native/foundation/src/` | Owns |
| --- | --- |
| `clipboard.rs`, `clipboard/`, `clipboard_transport.rs` | SQLite MIME bundles, pins/retention, projections, Wayland capture/replay |
| `apps.rs`, `application_roles.rs`, `actions.rs` | Validated launch plans, MIME role ownership, action metadata/routing |
| `volume.rs`, `control_center.rs`, `bluetooth.rs`, `bluetooth/` | Audio/network/device operations and confirmed readback |
| `transparency/` | Socket-activated TC30 room monitoring, stream ducking, ownership and crash recovery |
| `theme.rs`, `theme_render.rs`, `theme_runtime.rs`, `backdrop.rs` | Matugen palette mapping, generated outputs, immutable revision transaction, overview image |
| `wallpapers.rs`, `cache.rs` | Wallpaper catalogue/previews and bounded owned-cache maintenance |
| `deploy.rs`, `deploy_units.rs`, `preferences.rs`, `install.rs` | Installer sequencing, ownership journal, native session files, preferences and restore coordination |
| `update.rs`, `preflight.rs`, `source_identity.rs` | Isolated update/build validation and source identity |
| `monitor.rs`, `monitor_config.rs`, `monitor_shader.rs` | Kitty/btop panel lifecycle, persistent settings, appearance/motion |
| `shell.rs`, `session.rs`, `screenshot.rs` | Shell IPC, native session routes, capture |
| `browser_policy.rs`, `keyring.rs`, `system_setup.rs` | Narrow privileged integration contracts and reversible greetd/PAM installation |
| `local_ai.rs`, `app_mods.rs`, `geode.rs`, `spotify_theme.rs` | On-demand AI and scoped application adapters/modifications |
| `state.rs`, `process.rs`, `ui_preferences.rs`, `diagnostics.rs`, `doctor.rs`, `startup_report.rs`, `boot_report.rs`, `packages.rs` | Filesystem/lock primitives, finite subprocesses, accessibility preference read, diagnostics/cache |

### Communication and mutable state

QML `Process` objects send argument arrays to existing wrappers/native commands;
`StdioCollector` receives output. Most state-oriented commands return JSON.
Failures use nonzero exit status and stderr. Inspect the exact subcommand: some
return text, some consume stdin bytes/JSON, and Nothing is a line-oriented session.
Do not make callers parse friendly prose when a structured contract exists.

Backend -> shell feedback goes through `shell.rs` and the `foundation` IpcHandler
in `shell/shell.qml`. Shell IPC is not the same protocol as Niri's JSON EventStream.
The Niri adapter normalizes compositor state for UI; native IDs/operations stay
inside that boundary. Browser DevTools is not a general desktop IPC shortcut.

QML owns local focus/hover/selection and pending presentation. Rust owns validation,
actual settings, system effects and durable truth. Render checked state from
confirmed readback. Use stable item/device/request IDs; never identify a clipboard
bundle by preview text or a device by a list index.

Use `state.rs` for private owned directories, locks and atomic publication. Honor
HOME/XDG configuration, state, cache and runtime paths as the module defines;
never hardcode a reference user's home in reusable code. Validate absolute paths,
ownership and symlinks where required. Preserve unknown journal metadata when
compatible, and retain original backups across reruns.

`process.rs` already provides bounded output, deadlines and owned-process-group
cleanup. Use it for finite subprocess work instead of blocking QML or invoking a
shell with interpolated data. App/native-session handoff can use process replacement;
clipboard selection owners deliberately outlive a successful caller. Do not kill
all processes with a common executable name to stop one owned helper.

## 7. Quickshell surfaces and shared components

`shell/shell.qml` owns lazy loader lifetime, target output and IPC. Register a new
surface there only when the feature requires a new window; prefer an existing
view/group for related controls. Opening/closing state must remain separate from
whether a backend request can safely be destroyed.

Centered surfaces are Launcher, Clipboard, BluetoothPopup and Power. ControlCenter
is the left sidebar; the right monitor is Kitty/btop under native control. Volume
and Clock are non-focus-stealing readouts. OverviewBackdrop is a separate Niri
background integration, not the popup blur owner.

`SurfaceRouter` selects one visible/input-owning modal. `superseded` immediately
hides an outgoing window and releases its input while an owned request finishes.
A requested retiring target may wait for its own irreversible shutdown before
reopening. An unrelated destination must not wait for the old full exit animation.

`ModalBackdrop.qml` owns shared centered-popup dim/blur. It stays mapped across
clipboard/Bluetooth/session/launcher changes. Niri's rule for
`desktop-foundation-modal-backdrop` owns that blur; individual popup rules disable
independent blur. Keeping only QML scrim opacity is insufficient when the compositor
blur surface itself unmaps. The backdrop is input-transparent, below Overlay cards.
Its Niri placement and actual visual continuity require compositor-level validation;
an offscreen test can prove lifetime, not GPU blur output.

### Reuse before extending

- `SurfaceCard`: tonal card geometry, no shadow/effect layer.
- `ControlAction`: action/list/toggle presentation; compact and icon-only variants.
- `ControlVolume`: shared slider behavior; instant values, no interpolated volume.
- `ControlChoice`: provisional menu selection, commit/cancel, invoking-focus return.
- `ApplicationRow`: device/action/settings row, explicit selected/pending/confirmed state.
- `ClipboardRow`: clipboard presentation, pin and plain-copy affordances.
- `SearchInput`: real text editing; no global WASD/Space interception.
- `SelectionWheel`: selection-list gesture owner.
- `FocusIndicator`: common primary-colored keyboard outline.
- `SurfaceAnimation`, `SidebarMotion`: shared retargetable presentation/motion.
- `ControlRequest`, `ControlGroup`, `WallpaperGrid`, `SurfaceFooter`: existing
  request/group/grid/status abstractions; inspect their APIs before duplicating.

Keep loading geometry stable; commit coherent snapshots rather than revealing
partially discovered settings row by row. Empty, disconnected and error states
must retain a useful close/back route. Device disappearance must not retarget a
stale action to a new occupant of the same index. Do not scatter one-off color,
focus or wheel handlers through delegates to patch a shared defect.

## 8. Material, wallpaper and typography contracts

Google's official Material 3 Expressive specifications are authoritative. Use
[the reference catalogue](docs/MATERIAL_REFERENCES.md), including each applicable
component's overview, specs, guidelines and accessibility guidance. Android/Compose
examples are references, not dependencies to add to our desktop shell. If a page
requires JavaScript or its contents cannot be inspected, do not claim it was read.

`Theme.qml` loads the active semantic JSON with a repository fallback.
`Material.qml` exposes Material roles, type, shape, motion and spatial-focus helpers.
Use those tokens; do not independently re-derive wallpaper color in a widget.
Use on-role colors paired with their container roles, distinguish checked/focused/
selected/hovered/disabled/pending, and preserve contrast through transparency.
Do not use color alone to communicate a checked or failed state.

Google Sans Flex is the UI family. Use Google Sans Code/appropriate existing mono
configuration for code, terminal content and diagnostic/tabular exceptions.
Do not convert the terminal's normal monospace type to a proportional UI face.
Use shared type/shape/spacing tokens, responsive constraints and intended control
sizes. A slider does not need to consume the entire available row.

The source wallpaper fallback is `compositor/niri/wallpaper.toml`. The active
runtime transaction can retain its own `theme/current/wallpaper.toml`; do not
silently overwrite it from repository defaults on update. `wallpapers.rs` supplies
the home-relative wallpaper catalogue and cached previews to `WallpaperGrid`.

Native `theme.rs` runs the packaged Matugen executable once when extraction is
needed, then maps the chromatic palette onto Foundation semantic graphite roles.
`theme_runtime.rs` stages immutable revisions and manages `current`/`previous`;
`theme_render.rs` renders shell/application/Niri outputs. Hash-based palette and
image caching avoids unnecessary extraction. There is no permanent Matugen daemon.
Publication/reload failures retain or restore the last known good revision;
changes to shared rendering must preserve that contract.

Existing adapters have different refresh capabilities. See `theme/adapters.json`
and source: shell/Niri/Kitty, GTK native accent, stable Brave theme/policy files,
Spicetify color binding, btop preparation and next-launch consumers are not the
same reload path. Never restart a browser merely to recolor it or edit its active
profile databases. The root-owned browser-policy helper remains narrowly scoped.
A renderer/template edit may require deployment/`theme-runtime ensure`; a palette
cache hit alone does not prove the running generated configuration changed.

## 9. Keyboard, pointer and scroll behavior

Every interactive surface MUST support logical keyboard-only use. Arrows navigate;
WASD supplements them in non-text contexts; Enter/Space activate; Escape returns
one internal level or closes. Preserve Tab/Shift+Tab routes to every action,
including pin, clear, cancel and dropdowns. Initialize focus at search or the most
useful enabled device/action. Restore invoking focus when returning from a menu.

Text editing owns letters, Space, selection and Ctrl shortcuts. Search surfaces
explicitly move focus into results before treating WASD/Space as actions. Do not
install a global `Keys.BeforeItem` handler that consumes editor input. Sliders
adjust horizontally and offer a vertical/Tab escape; Space must not trigger an
unrelated parent action. Ignore auto-repeat for activation/destructive toggles;
retain repeat where continuous adjustment is intended.

The actual shortcut source is `compositor/niri/bindings.kdl`. Check it before
assigning a chord; preserve unrelated local edits. Current key ownership includes
Super+Space launcher, Super+V clipboard, Super+B Bluetooth, Super+Shift+Q session,
Super+Z control center, Super+X monitor, Super+Y local AI, Super+C manual column
centering and Super+Shift+C clock. Toggle bindings use `repeat=false`.
Do not steal Super+C for a new widget. Niri keeps `center-focused-column "never"`
with `always-center-single-column`; focusing multiple columns must not force center.

One wheel event has one owner. Bluetooth/Nothing previously installed handlers on
both a full card and its nested list. The fix is a handler on the actual list,
with gesture residue discarded on disable/end. Scrolling must not adjust an
unrelated control or activate another list as the pointer moves. Content passing
under a stationary pointer must not override keyboard/wheel selection; pointer
selection follows genuine movement, not delegate creation.

Use proper drag ownership, hover/press state layers, disabled behavior, accessible
names and visible focus. A modal traps input only while it owns that interaction;
readouts and an unfocused non-modal sidebar must not steal focus from applications.

## 10. Motion and lifecycle regression rules

The detailed state machine is in the UI standard. Centralized durations are in
`Theme.timing` and `Material.motion`; use `SurfaceAnimation` rather than another
local default. Current Bezier curves are desktop approximations of Material motion,
not exact physical springs. Google's Expressive motion uses spatial/effects spring
families; document deviations instead of mislabeling existing timing as official.

Sidebar geometry/motion is shared: 720px reference width, full height, 12px padding,
14px inner corners, 75% background opacity and 240ms/180ms open/close. Preserve
Niri-aware responsive bounds and existing behavior. Volume changes stay immediate;
only its presentation fades for 100ms. Never animate every volume sample behind
keyboard repeat.

Historical regressions MUST remain covered:

| Regression | Verifiable cause and required prevention |
| --- | --- |
| Sidebar exit never finishes | Router called dismiss every 16ms and ControlCenter restarted motion. Do not retarget an already-closing exit; dismissal is idempotent. |
| Slow popup-to-popup switching | Waiting for old exit before new entrance chained delays. Hide outgoing presentation immediately and show the destination, retaining owned backend cleanup. |
| Blur disappears during handoff | Per-popup compositor blur vanished with the old Wayland surface. Keep shared `ModalBackdrop` mapped; do not substitute a new dim rectangle for blur continuity. |
| Rapid reopen later vanishes | Obsolete exit timer destroys the reopened loader. `present()` cancels old close completion and retargets from current values. |
| Hidden sidebar exit stalls | A mapped frame clock cannot be assumed after hiding. Superseded ControlCenter motion settles without waiting for visible frames. |
| Unresponsive btop toggles | `monitor.rs` must preserve latest intent through motion reversal, retirement and native btop save/shutdown. Do not bypass graceful saving or restart Kitty on every toggle. |
| Clipboard copy does not close | A success callback can run before process running/busy is cleared. Schedule dismissal after completion handling; do not close on failed copy. |

`ui_preferences.rs` reads the desktop animation preference on demand with a bounded
subprocess; `Theme.reducedMotion` centralizes the QML response. Reads happen at
startup/surface requests, not through a permanent polling worker.
`DF_REDUCED_MOTION=1` is an explicit override. Test zero-duration completion, not
only normal animation. Do not claim this automatically covers every independent
external renderer or the compositor's own animations.

## 11. Performance and external process lifecycle

MUST keep the QML input thread responsive. Use asynchronous processes and bounded
native operations; no sleeps, image decoding, synchronous network access or long
filesystem scans in input handlers. Expensive palette/image/catalogue work belongs
in Rust and caches, not in delegate bindings or repeated shell commands.

Prefer observable improvements: distinguish input-to-state latency, animation lag,
startup work, IPC delay and application shutdown time before optimizing. Reuse
already-published state; avoid refreshing all devices for every volume tick.
No permanent process or polling loop should be added merely for convenience.

The monitor is `monitor.rs` + `monitor_config.rs` + Kitty layer-shell + btop, not
a QML terminal emulator. Persistent sidebar settings honor XDG_CONFIG_HOME and
must not be overwritten on each launch. Preserve save-on-exit, the ownership of
the exact child process, non-modal focus, latest-toggle intent and bounded cleanup.
A hidden retiring process is not permission to leave a permanent idle worker.

Keep temporary transition timers finite and conditional. Stop periodic control
center discovery when closed; retain only work already owned and completing
safely. Avoid unnecessary QML object recreation and binding loops. A warning that
appears only on live Wayland remains a real issue even if an offscreen fixture passes.

## 12. Installation, update, backups and restoration

### Entry points and boundaries

`scripts/install-all` invokes the personal installer path; `scripts/install`
delegates to native `install.rs`. The wrapper builds a missing backend only for
an actual install when Cargo is available; inspection commands report the build
prerequisite without compiling or creating state. Package manifests live
under `packages/`. `scripts/bootstrap` is the CachyOS package bootstrap, including
optional manifest groups. Privileged session/boot/browser-policy setup is separate
from a normal-user desktop update. Do not execute an installer just to validate prose.

Native `scripts/deploy` delegates to `deploy.rs`. It requires built backends,
validates configuration, generates native session units/local-AI launcher files,
coordinates preferences and records managed links. `--update` skips global
preference writes. `scripts/preferences` is native preference setup.
`scripts/uninstall` restores recorded preferences/roles and deployment-owned paths;
packages remain installed. Its optional `--greeter` route invokes the reviewed
privileged system-setup restore. The `system-setup` entry point is now native Rust,
requires UID 0, and accepts only `install` or `restore`; it has no alternate-root
or owner override for protected system paths in the production CLI. The common
`--root` option selects source checkout assets only. Its existing root journal is preserved.

### Ownership and recovery

`$XDG_STATE_HOME/desktop-foundation/manifest.json` records the owning checkout,
managed paths, original fingerprints, backup paths and pending changes. Defaults
follow XDG conventions. Originals are recorded before replacement and retained
across reruns. Preserve compatible unknown metadata. Validate the whole restoration
plan before mutating it; an externally replaced path or missing backup is a
conflict, not permission to overwrite or discard the journal.

Use atomic writes/symlink publication and the existing transaction locks. Preserve
recovery boundaries on interruption. A failed update must not uninstall the user's
working desktop as a side effect. Do not delete backups to make a rerun pass.
Source deployment rollback and runtime theme rollback are different transactions.

### `df-update`

The installed `~/.local/bin/df-update` and checkout `scripts/df-update` use
`update.rs`. They update Desktop Foundation itself, not system/AUR packages.
The updater validates the owning checkout and origin, fetches, preserves local
settings/files, merges/builds in an isolated Git worktree, validates, publishes
the accepted result, redeploys and reloads active Foundation components.
Tracked changes receive a reachable backup under `refs/desktop-foundation/`;
do not replace this workflow with `git reset --hard` or `git clean`.

`--dry-run` is an inspection path. Do not accidentally run `df-update` on the next
line of a supposed dry-run example. Conflicts/failures must preserve the current
checkout/settings and report a useful recovery route. Keep routine terminal output
concise; verbose build/Git output should be available when diagnosing failure,
not a permanent success transcript. Inspect the published configuration and reload
result before claiming deployment succeeded; queued IPC alone is not acceptance.

## 13. Python migration policy and current limitations

MUST NOT introduce new Python in production, tests, installation tools or automation.
Migrate connected behavior to Rust while preserving wrappers/contracts and journals.
Do not remove an import-compatible module until remaining callers and tests have
a replacement. Do not execute Python to implement a supposedly Python-free change.

**Current reality:** the repository is not Python-free. Legacy privileged
provisioning, older render/helper modules, some client provisioning,
terminal `select_all.py`, benchmark tooling and older tests remain. The public installer, login setup, doctor,
startup timeline, boot reports and session-acceptance snapshot now run in Rust. The obsolete
`install_engine.py` and its import tests have been removed. CI still
compiles/runs Python tests and installs Python tooling. `scripts/dev bench` is
still a Python route. These are tracked migration targets, not authorized patterns
for new work. Normal native deployment/theme/update and the native/offscreen test
workflow have already moved away from those paths.

Likewise, explicit platform gates remain despite the revised compatibility policy.
Some older specialized docs/research refer to historical compositor routes or
previous styles. Verify them against active dispatchers. Existing duplicate-signal,
binding-loop or Wayland-focus warnings are not silently certified by a successful
shell load. No new full sidebar, browser refresh capability, fresh-install result
or universal Material compliance should be described as implemented without evidence.

## 14. Safety, privileges and system integration

- MUST NOT run live disruptive acceptance tests: actual power/session actions,
  package updates, Bluetooth changes, audio changes or clipboard-history cleanup
  are not test fixtures. Use isolated directories, fake tools/transports and
  offscreen QML. Applying an authorized update is distinct from exercising controls.
- Preserve running applications, user data, pairing/account state and sessions.
  Never silently restart Niri, greetd or the login session to validate a change.
- No blanket passwordless root. Let the user authenticate interactive sudo in
  their terminal when a protected change is needed; never collect their password.
- Privileged writers must be root-owned with narrow validation/permissions. Never
  make `/etc/.../policies/managed` world-writable for browser theming.
- Do not edit active browser Preferences JSON, profile/extension databases,
  cookies, history, credentials or sessions. Use the existing supported policy
  helper, with its explicit limitations.
- oo7 is the chosen secret service; retain PAM/greetd readiness integration and
  existing keyring security. Do not add GNOME Keyring by habit or blank passwords
  to avoid an unlock prompt. Diagnose login/token/session readiness instead.
- Local AI is on demand through the existing launcher/configuration. Do not
  enable Ollama/Odysseus at boot or change service/firewall exposure as a side effect
  of a UI change. Consult `docs/local-ai.md` and preserve user edits there.
- Missing hardware/service/capability must produce a meaningful disabled/error
  state, not a crash, infinite spinner or fake success.

## 15. Verified development commands and test boundaries

Run from the repository root. These names exist; still inspect flags/side effects
before use. No command below is a blanket instruction to mutate the current desktop.

```sh
# Read working state and inspect relevant source.
git status --short
git diff --check

# Build release backends (no deployment).
./scripts/build-backend
./scripts/build-nothing

# Native static checks and tests.
cargo fmt --manifest-path native/foundation/Cargo.toml --check
cargo clippy --locked --manifest-path native/foundation/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --locked --manifest-path native/foundation/Cargo.toml
cargo test --locked --manifest-path native/nothing/Cargo.toml

# Production QML controls in an isolated, offscreen Quickshell host.
./tests/test-material-widgets

# Repository formatting/lint/native validation; requires its declared tools.
./scripts/check
./scripts/test

# Inspect an update/deploy plan without applying it.
./scripts/df-update --dry-run
./scripts/deploy --compositor niri --profile default --dry-run
```

`tests/test-material-widgets` creates private fixture state, removes display/session
bus variables, substitutes inert clipboard work and uses offscreen Qt. It tests
production controls with Wayland transport replaced. Tests must fail on errors,
not print an unconditional pass after a thrown QtTest assertion.

Native integration targets include `clipboard`, `clipboard_pin_isolation`,
`clipboard_transport`, `deploy`, `preferences_setup`, `theme_runtime`, `wallpapers`,
`cache` and `process` under `native/foundation/tests/`. They cover durable state,
fake Wayland protocols, pin independence, missing backups, interruption, theme
publication and restoration. Inspect fixture isolation before adding system calls.
Some deployment fixtures are reference-platform conditional; skipped tests are
not a passing cross-platform installation result.

CI in `.github/workflows/check.yml` runs portable Rust checks/builds, shell/Fish
checks and the remaining Python suite on Ubuntu. It does not run a physical Niri
session or certify wallpaper blur, hardware, login or actual package installation.
Do not run/extend the legacy Python suite for new Rust/QML work; migrate it when
that path is in scope. Report exactly which checks ran and their limits.

For an authorized shell update, `scripts/shell-reload` asks the existing shell to
reload; then inspect its service journal for **Configuration Loaded** or errors.
An active service or a responsive old IPC handler alone can conceal a failed
reload that retained the previous configuration. Niri validation checks generated
configuration, not visual blur continuity or focus behavior on physical outputs.

## 16. Task-oriented implementation workflows

| Task | Inspect / change | Required verification focus |
| --- | --- | --- |
| Create a widget | UI standard, shared controls, `shell/shell.qml`, router/IPC, existing bindings, backend action contract | Lazy ownership, keyboard-only routes, empty/error/loading, safe open/close/reopen, no idle work |
| Modify a widget | Actual surface, its shared primitives and callers; stable IDs/state | Existing functions, pointer vs keyboard ownership, focus return, long/empty/disconnected data |
| Change Material tokens | Official spec/reference index, `Material.qml`, `Theme.qml`, native palette/rendering and fallbacks | All consumers, role contrast/state distinction, no local substitute colors, documented adaptation |
| Implement animation | Shared motion/geometry tokens, close completion, mapped-window assumptions | Interruption, reversal, rapid cross-widget switches, no repeated restarts, zero-duration completion |
| Add keyboard interaction | Actual binding ownership, editor exceptions, focus chain and control-native keys | Arrows/WASD, Enter/Space, Tab/Shift+Tab, Escape, text editing, auto-repeat |
| Add backend command | `main.rs`, `lib.rs`, nearest owning module, wrappers, JSON/exit contract | Argument validation, output/deadline bounds, fake tools, failure/readback, no unintended privilege |
| Change installation/update | Dispatcher, journals/locks, XDG paths, updater staging and source identity | Rerun, external ownership conflict, interruption, retained originals/local changes, dry-run safety |
| Migrate Python | Complete caller/import graph, old data/schema/CLI, nearest Rust owner | Native parity on fixtures, journal compatibility, wrappers retargeted, no remaining active Python dependency claimed away |
| Fix regression | Small reproducer, last relevant diff, actual event/order/lifetime | Failing isolated case before fix where possible; ensure neighboring interactions still work |

A fix belongs at the narrowest layer that owns the defect. Use this decision table:

| Situation | Preferred location |
| --- | --- |
| Same visual/input defect across widgets | Existing shared QML component/token |
| Widget-local navigation/focus problem | Surface or presentation helper |
| Popup handoff/ownership problem | Composition root/router/backdrop, not each shortcut wrapper |
| Device/file/system behavior | Existing Rust domain module |
| Durable file/recovery behavior | Owning transaction using `state.rs` |
| New privilege need | Reviewed scoped helper; no broad sudo/shell shortcut |
| Unsupported environment | Capability diagnosis; no invented compatibility promise or new name gate |

## 17. Agent workflow, Git and completion requirements

1. Read this file, the UI standard if applicable, working-tree status, affected
   implementation and relevant specialized docs. Search with `rg` first.
2. Identify state/process owners and source-of-truth files before editing. Do not
   patch only deployed generated files when the repository renderer is responsible.
3. Preserve unrelated edits, untracked user files, settings and original backups.
   Do not stage a whole tree without reviewing what it includes.
4. Make the smallest connected correction, including shared callers and regression
   coverage. Do not broaden a documentation task into unrelated application rewrites.
5. Run appropriate tests, inspect output and review the diff. Separate syntax,
   fixture behavior, compositor validation and live visual acceptance in reports.
6. Apply only already-authorized changes. Publishing, destructive actions, session
   termination and changing unrelated privileged policy require explicit authority.
   A user request to implement a feature is not permission to exercise its dangerous
   real-world action as a test.
7. Commit task-owned changes locally. Prefer separate focused documentation and
   behavior commits when both are requested. Never push/tag/release without a request.
8. Report outcomes, relevant checks, material gaps, commit hash and working-tree
   state. Do not claim a test passed when not run, a queued reload succeeded before
   inspecting it, or an offscreen fixture proved compositor rendering.

## 18. Prohibited patterns and review checklist

MUST NOT:

- Add Python or extend a legacy Python implementation because it is nearby.
- Copy an existing widget to create a competing style/input/motion system.
- Hardcode an arbitrary theme color, add shadows or replace UI typography locally.
- Advertise standard navigation through permanent decorative instruction text.
- Capture WASD/Space from editors or leave a secondary action keyboard-inaccessible.
- Put identical wheel handlers on a list and ancestor card.
- Restart closing motion on every router tick or destroy a reopened loader from
  a stale timer. Chain unrelated exit/entrance delays or unmap shared blur mid-handoff.
- Block an input handler while launching/scanning external applications.
- Overwrite btop settings at launch, bypass save-on-exit or kill arbitrary Kitty processes.
- Reset/clean the user's checkout, overwrite externally replaced managed paths,
  discard backups or silently reinstall packages during `df-update`.
- Reject another distribution solely by name in new capability-compatible code.
- Suppress a useful error, fabricate readback or treat a disabled device as connected.
- Invent source paths, command flags, APIs, official Material measurements or
  capabilities inferred only from a third-party design library.
- Describe planned work as implemented, or use documentation to conceal an existing gap.

Before completion, check: ownership preserved; shared architecture respected;
interaction and failure paths covered; relevant tests actually passed; generated
outputs and reload verified when applied; unrelated edits still present; only
focused files staged; no publication without authorization.
