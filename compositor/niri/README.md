# Future Niri integration

Planning only; no Niri implementation. Replace the adapter selected in shell/shell.qml with a Niri implementation of docs/compositor-interface.md. Use Niri event-stream IPC, maintain compositor-native workspace/window IDs as opaque strings, and preserve capability differences. Do not assume fixed numbered workspaces, Hyprland addresses, or identical scrolling semantics. Niri overview belongs to the compositor capability; a future shell overview would be a separate on-demand surface.
