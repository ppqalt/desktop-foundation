# Desktop removal audit — 2026-10-01

No packages removed. Sudo requires user authentication; post-change cold login
is still pending. greetd/tuigreet already ran this boot; COSMIC Greeter is disabled.

Keep `cosmic-files`: it is the current inode/directory default used by Super+E.
Keep `hyprpolkitagent`: active working Niri authentication agent. Keep its Hypr
libraries, all installed fonts, Niri, GNOME/GTK portals, keyring, Bluetooth/network
and audio support. Retained repository Hyprland implementation does not require
its runtime packages on a Niri-only host; bootstrap now has `--hyprland` opt-in.

Explicit/dependency install reasons and reverse dependencies were inspected.
The following nonrecursive removal simulation succeeds and preserves every shared
package. This is the recommended first cleanup after cold-login acceptance:

```sh
sudo pacman -R cosmic-app-library cosmic-applets cosmic-bg cosmic-comp cosmic-greeter cosmic-idle cosmic-initial-setup cosmic-launcher cosmic-monitor cosmic-notifications cosmic-osd cosmic-panel cosmic-player cosmic-randr cosmic-screenshot cosmic-session cosmic-settings cosmic-settings-daemon cosmic-sound-theme cosmic-terminal cosmic-text-editor cosmic-wallpapers cosmic-workspaces hyprland hyprpaper uwsm
```

For comparison, the requested recursive form (NOT executed) is:

```sh
sudo pacman -Rns cosmic-app-library cosmic-applets cosmic-bg cosmic-comp cosmic-greeter cosmic-idle cosmic-initial-setup cosmic-launcher cosmic-monitor cosmic-notifications cosmic-osd cosmic-panel cosmic-player cosmic-randr cosmic-screenshot cosmic-session cosmic-settings cosmic-settings-daemon cosmic-sound-theme cosmic-terminal cosmic-text-editor cosmic-wallpapers cosmic-workspaces hyprland hyprpaper uwsm
```

That recursive transaction would ALSO remove these currently dependency-marked
packages:

- `acpid`
- `adw-gtk-theme`
- `cdparanoia`
- `geoclue`
- `gst-plugins-base`
- `gst-plugins-good`
- `hyprcursor`
- `hyprland-guiutils`
- `hyprwire`
- `libnma`
- `libqalculate`
- `muparser`
- `nm-connection-editor`
- `playerctl`
- `pop-launcher`
- `pugixml`
- `python-pyxdg`
- `qt6ct`
- `re2`
- `switcheroo-control`
- `tomlplusplus`
- `wavpack`

Do not run the recursive command as-is: Playerctl is used by End play/pause,
Qt configuration and networking editors remain useful, and media/geolocation
support needs individual review. The nonrecursive command avoids this collateral
removal. No artificial package-count target and no automatic orphan cleanup.

`pacman -Qtdq` reported no orphans at audit time. After any removal, inspect it
again and check reverse/optional dependencies before removing candidates. Optional
uses are not protected by pacman's dependency solver. `pacman -Qqe` and each target's
`pacman -Qi` output were inspected, not inferred from original install grouping.

Root changes still required: install missing bootstrap packages and optional
PAM hooks through the tracked system installer. Do not disable the keyring socket.
