# Menu surfaces

Launcher, clipboard, Bluetooth, earbud controls and session menus share card
styling, rows, keyboard hints and animations. Their backgrounds dim and blur the
applications beneath them.

Arrows, Tab and the wheel select; Enter or a click activates. Escape closes or
returns to the parent page. Escape and action hints are clickable. Search fields
keep their text editing controls. Menu shortcuts fire once per press.

Bluetooth selects the top paired device when opened. Earbud controls reveal the
complete settings together, retain reported values while saving, and highlight
confirmed changes. Switches and value badges stay visible.

The session entrance starts on its own first frame. Shortcut closure goes through
the same exit animation as Escape. Clipboard's clear dialog uses a compact size
transition. Volume has a 100 ms fade in/out with instant bar and percentage updates.

After replacing QML source, run `scripts/shell-reload` to load the completed
update. It waits briefly for the shell and defers during a pending session action.
