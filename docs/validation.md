# Validation on Tops — 2026-09-30

Hyprland 0.56.2, Quickshell 0.3.1, Qt 6.11.2, native scrolling layout. One active 1920×1080 output at ~144 Hz, scale 1, selected through automatic monitor configuration. The current session is ordinary Hyprland, not UWSM managed.

Passed:

- Rust stable/compiler/Cargo/rustfmt/Clippy/rust-analyzer version checks. No crate was added, so no fictitious backend build/test is claimed.
- ShellCheck and shfmt checks on all shell scripts; qmlformat comparison and qmllint on every QML file; luac syntax checks; Python AST checks. The only narrow lint suppression is Quickshell's runtime-creatable PanelWindow, which was verified live.
- Hyprland --verify-config before deployment and after deployment, live reload with empty configerrors, scrolling layout confirmed by getoption, automatic monitor handling confirmed.
- The deployed configuration registered temporary Kitty and exit/focus/movement bindings. The real launch path opened two owned Kitty windows. Live adapter IPC exercised workspace focus, window focus, scrolling column swap/view movement, moving a targeted window to another workspace, and graceful targeted close. State was checked after events; original focus was restored and test windows removed. This validates dispatch paths, not physical keyboard input.
- Fresh Quickshell process immediately reports the already-focused window without requiring a subsequent focus event. Native Hyprland events and standard Wayland toplevel subscription stay inside the adapter.
- Probe rendered visibly; screenshot inspected. Workspace and focused-title text were correct. Hide removed the layer-surface and emitted PROBE_DESTROYED. Startup leaves no layer-surface or visible chrome. No polling timer, subprocess loop or Rust daemon exists in the resident shell.
- Deployment repeated on Tops without replacing original backups. Isolated filesystem test exercised original-file backup, repeat installation, refusal to overwrite externally modified paths, restoration, repeat restoration and missing-profile refusal.
- Packaged hyprpolkitagent, xdg-desktop-portal and xdg-desktop-portal-hyprland are active. The active-session startup path succeeds and duplicate shell launch is prevented. Session-exit routing was checked with isolated command substitutes, including an absent shell; the actual logout dispatcher was deliberately not executed.
- COSMIC executable/session entry remain installed; no COSMIC configuration/packages were changed. Git uses logical commits and a repository-local agent identity.

Observed issues and resolution: the initial probe shadowed its injected adapter name; fixed and visually tested. Installed Quickshell qmltypes expose native HyprlandToplevel objects where generated online docs describe generic Toplevels; the adapter follows the installed API and normalizes hexadecimal IDs. Initial focus required explicitly initializing the standard Wayland toplevel subscription. Kitty's normal confirmation for closing a running test command was handled by disabling confirmation only for the two validation windows; user Kitty configuration was untouched.

Resource measurements (each finite 10-second sample, shell hidden):

| State | RSS MiB | PSS MiB | CPU % of one core | Main-thread voluntary/involuntary switches per second |
| --- | ---: | ---: | ---: | ---: |
| Fresh process, probe never created | 110.5 | 75.8 | 0.0 | 0 / 0 |
| Probe shown then destroyed | 166.0 | 115.5 | 0.0 | 0 / 0 |

Zero measured ticks/switches in these short samples does not prove zero CPU/wakeups. Main-thread switches are not all-thread or hardware wakeup counts. The memory increase is consistent with retained rendering resources/caches, but allocation ownership was not profiled; hidden-after-rendering must be measured separately; no premature optimization was applied. One launch-to-backend-ready sample was 128.5 ms. Hyperfine's whole startup/teardown harness averaged 160.7 ± 2.6 ms across five warm runs (157.5–163.5 ms); it includes Python/IPC/teardown overhead and is not pure render latency. First-frame/animation/blur costs await an actual designed surface.

Remaining verification limits: no full UWSM login/logout or COSMIC fallback login was performed because ending the current desktop would interrupt work. At the next login select **Hyprland (uwsm-managed)**; verify user graphical-session target, portal/polkit, Kitty and shell, then save work and use Super+Shift+M before selecting COSMIC. Multi-monitor hotplug, alternate GPUs and lucky38 hardware are not yet tested. Optional perf installation needs an interactive sudo password; no perf traces, exact wakeup counts or QML frame profiles are claimed. Bootstrap is ready for that optional installation.

Foundation phase ends here. Choose one individual feature/surface before adding any desktop UI.
