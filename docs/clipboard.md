# Clipboard

Super+V opens searchable text and image history. Arrows, Tab and the wheel select
an item; Enter copies it and closes the menu. Ctrl+Delete removes the selected
item. Clear all is reachable with Shift+Tab or Up from the first result,
Down from the last result, or Ctrl+Shift+Delete.

The clear dialog has Cancel and Clear history buttons. Tab or arrows select,
Enter/Space activates and Escape cancels. The dialog owns keyboard input until
it closes.

The Rust backend stores history in SQLite and publishes a watched index for QML.
Identical payloads are deduplicated. Images are stored as original clipboard bytes.
History lives under `$XDG_STATE_HOME/desktop-foundation/clipboard`.

```sh
scripts/foundation clipboard init
scripts/foundation clipboard clear
```

Clipboard watchers start with the desktop session. The persistence process keeps
the current selection available after its source application exits.
