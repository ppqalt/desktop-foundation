# Tooling on Tops, 2026-09-30

The requested pacman development packages, compositor/session/portal/polkit packages and Kitty were already installed. This phase installed Rust stable through existing rustup, with rustfmt, Clippy and rust-analyzer; no pacman transaction succeeded or package was removed. There is no conflicting distro Rust toolchain: rustup owns the Rust commands.

Verified: rustc/cargo/rust-analyzer 1.98.1; rustfmt 1.9.0-stable; Clippy 0.1.98. Qt tools are 6.11.2, in /usr/lib/qt6/bin: qmllint, qmlformat, qmlls, qmlprofiler and qsb. The installed Quickshell package includes qmldir/qmltypes metadata and its Hyprland event integration. Development commands use explicit Qt paths without changing global PATH. Native profile tools hyperfine, heaptrack, gdb and strace are installed. Lua syntax validation is available through luac; Python handles reversible deployment and finite measurement utilities.

Optional perf is available in the official extra repository but remains uninstalled: sudo requires an interactive password. Run scripts/bootstrap interactively to install it; review pacman's upgrade proposal. No kernel profiling permissions were relaxed. Qt Creator GUI is optional and was not installed; qmlprofiler CLI itself is present and its help/version were verified. No Qt trace was captured in this empty phase.

Installed package inventory:

```text
git 2.55.0-1.1
base-devel 1-2
rustup 1.29.1-1.1
clang 22.1.8-2
llvm 22.1.8-2
lld 22.1.8-1.1
cmake 4.4.3-2.1
ninja 1.13.2-3.1
pkgconf 3.0.7-1.1
quickshell 0.3.1-1.1
qt6-declarative 6.11.2-2.1
qt6-tools 6.11.2-1.1
qt6-shadertools 6.11.2-1.1
shellcheck 0.11.0-142
shfmt 3.14.1-1.1
jq 1.8.2-1.1
hyperfine 1.20.0-1.1
heaptrack 1.5.0-11.1
gdb 17.2-1.1
strace 7.2-1
kitty 0.49.1-1.1
hyprland 0.56.2-3.1
uwsm 0.27.0-1
xdg-desktop-portal-hyprland 1.4.1-2.1
hyprpolkitagent 0.2.0-1.1
dbus 1.16.2-1.1
python 3.14.7-2
lua 5.5.1-1.1
```
