# Shell services

`Applications.qml` adapts Quickshell's desktop-entry watcher and parsed launch
commands. `ClipboardHistory` reads the Rust backend's watched history index.
Bluetooth state belongs to its popup. Menu loaders release these objects after
the closing animation.
