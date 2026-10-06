# Clipboard

Super+V opens searchable history for copied text, formatted text, images, files
and folders. A group of copied files stays one item. Enter copies the selected
item and closes the menu; a paste uses the formats accepted by the destination.

## Keyboard controls

| Key | Action |
| --- | --- |
| Up/Down, Ctrl+P/Ctrl+N, PageUp/PageDown | Select a history item |
| Tab / Shift+Tab | Move through the selected item's actions and Clear history |
| Enter | Activate the focused action |
| Ctrl+Shift+P | Pin or unpin the selected item |
| Ctrl+Shift+C | Copy text without formatting |
| Ctrl+Delete | Delete the selected item |
| Ctrl+Shift+Delete | Open the clear dialog |
| Escape | Cancel a dialog or close the menu |

Search matches copied text, filenames and format labels. Pinning keeps the item
at the top and preserves selection when the list reorders. Row actions are also
available with the mouse.

## Pins and clearing

Pinned items survive ordinary history eviction and **Clear history**. Use
**Clear everything** in the clear dialog to include pins. Deleting an individual
item is explicit and also works for a pin.

Ordinary history retains up to 100 items and 32 MiB. Pins have their own limit of
100 items and 32 MiB; exceeding that limit reports an error instead of discarding
an existing pin. Each stored representation is limited to 8 MiB, with 32 MiB per
combined item.

## Files and formatted text

Copied file lists retain their original locations. Videos, images, archives,
code files, folders and mixed groups use the same file-list behavior; file
contents are not duplicated into history. Moved or deleted originals are marked
unavailable. Restoring an old cut through history pastes it as a copy.

Copying `.md` or `.toml` files preserves the files. Copying their contents stores
text. Formatted-text items keep offered HTML/RTF together with plain text; normal
copy preserves formatting where the destination supports it. Copy without
formatting uses the plain-text version.

PNG, JPEG, GIF and WebP image data can also be stored directly. A copied image
file is a file-list item, separate from copied image pixels.

## Backend

One event-driven Rust watcher captures supported formats from the same Wayland
offer. A new selection cancels unfinished transfers from the previous offer.
Replay publishes those formats together and confirms clipboard ownership before
reporting success. The serving process exits when selection ownership ends.

SQLite history lives under
`$XDG_STATE_HOME/desktop-foundation/clipboard`. Schema updates retain existing
items. QML reads a watched index and handles presentation and navigation.

```sh
scripts/foundation clipboard init
scripts/foundation clipboard pin ITEM_ID
scripts/foundation clipboard unpin ITEM_ID
scripts/foundation clipboard copy ITEM_ID
scripts/foundation clipboard copy-plain ITEM_ID
scripts/foundation clipboard clear
scripts/foundation clipboard clear-all
```

`init` refreshes the index, including file availability. Session startup runs the
watcher; clipboard persistence keeps a selection available after its source exits.
