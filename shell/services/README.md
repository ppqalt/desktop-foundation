Services are event-driven state owners. Applications.qml adapts the native
DesktopEntries watcher and parsed-command launch path; it is owned by the lazy
launcher, with no polling. The compositor adapter remains separate. Add other
providers only when their feature is requested. ClipboardHistory is owned by the
lazy clipboard surface; Bluetooth state is owned by its lazy popup. Their loaders
release UI objects after closing animations. Never put command launches or
polling in a surface binding.
