# Foundation architecture

Hyprland owns window management, scrolling layout, input, output configuration and compositor effects. Quickshell owns future visual surfaces. The resident shell currently creates no visible chrome. Its temporary probe is instantiated through LazyLoader on request and destroyed when hidden. Native event subscriptions provide compositor state without polling or command chains. There is no Rust daemon, no launcher, panel, notification service or invented desktop feature.

Shared compositor configuration and Finnish keyboard preferences (compositor/hyprland/input.lua) are separate from host hardware/monitor profiles. Optional host input-device overrides have their own input.lua file; the shared layout does not depend on host selection. Tops uses automatic preferred outputs with automatic scale; no output name or GPU is in shared files. To port to lucky38, create profiles/lucky38 with the three Lua files (or deploy default), then run the deployment script with that profile. Repository paths are resolved at deployment and startup, not hardcoded into shared source.

UWSM should manage future login sessions; use the installed Hyprland (uwsm-managed) session. Its startup is finalized once, and applications launch through uwsm app. Ordinary Hyprland is also supported. Portal DBus activation uses the system's existing desktop selection; no persistent global environment configuration or service replacement. hyprpolkitagent starts only from the Hyprland startup path and is not globally enabled. COSMIC packages/configuration/session entries are untouched.

## Hardening additions

The shell is a runtime-only systemd user service with bounded restarts. Session
startup owns environment import and polkit independently; application launch does
not depend on shell health. Deployment journals intent before each rename/link.
The composition root alone wires the Hyprland adapter and lazy diagnostic probe.
Normalized snapshots expose outputs, workspaces, windows, focus and explicit
window state; native refresh happens only for relevant events, coalesced once per
event-loop turn. Debug counts/storm logging are off by default and have no timer.
Theme tokens remain temporary centralized placeholders. `shell/services` and
`shell/utils` document extension boundaries without speculative providers.
See [Niri planning](niri-port.md) for an event-stream adapter design. There is no
Rust daemon and no final desktop surface. Unsupported compositor startup fails
clearly with bounded service retries; it cannot exit or restart the compositor.
