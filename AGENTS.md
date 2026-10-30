# Desktop Foundation agent instructions

Read [docs/FOUNDATION_UI_STANDARD.md](docs/FOUNDATION_UI_STANDARD.md) **before
changing any window, widget, menu, popup, overlay, shared control, input handler,
theme token or UI/backend contract**. It is the repository's normative UI
engineering standard, not optional design inspiration.

## Project constraints

- Support **CachyOS and Niri only**. Archived compositor sources are not another
  supported path. Preserve restoration of existing deployment journals.
- Use **Rust for backend behavior and QML for presentation**. Add no Python code,
  tools or tests. Migrate legacy Python callers in connected, reversible batches.
- Use Google's official **Material 3 Expressive** documentation as authority.
  Read the relevant current specifications; do not invent measurements or call
  approximations exact. Record necessary desktop adaptations in the UI standard.
- Reuse shared components and tokens. Fix a shared primitive before adding a
  surface-specific workaround. No shadows, including under windows/sidebars.
- Preserve wallpaper-derived semantic colors, Google Sans Flex UI typography,
  the barless desktop and current shortcuts. Check bindings before assigning any.
- Keep labels minimal; retain meaningful errors, status and confirmations.
- Do not add unrelated user-facing features during UI standardization.

## Work and verification

- Read current source and working-tree state first. Preserve unrelated edits,
  user settings, ownership journals and original backups.
- Never run live disruptive acceptance tests: no actual session/power actions,
  system updates, Bluetooth changes, or clipboard-history cleanup for a test.
  Use isolated Rust fixtures and offscreen QML interaction tests. Applying an
  authorized desktop update is separate from exercising live controls.
- Do not edit active browser profile databases or account/session data. Retain
  the narrow root-owned policy writer for supported browser theme updates.
- No blanket passwordless root. Let the user enter interactive sudo credentials
  when protected changes are needed; never request their password.
- Run checks appropriate to changed behavior; report practical limits honestly.
  Do not claim whole-desktop acceptance from a single component test.
- Commit only task-owned files. Do not push or publish without a request.

## Starting points

- `shell/shell.qml`: lazy surface composition, IPC and modal handoff.
- `shell/components/`: reusable input, layout, motion and surface primitives.
- `shell/theme/`: generated palette access and shared Material/desktop tokens.
- `native/foundation/src/`: backend commands and durable state ownership.
- `compositor/niri/`, `theme/templates/niri.kdl.in`: compositor integration.
- `tests/test-material-widgets`: inert offscreen production-component checks.
