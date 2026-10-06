# Application defaults

`config/application-roles.json` maps each role to a package, executable, desktop
entry and MIME types. Shortcut launch commands use the selected role.

| Role | Application |
| --- | --- |
| Terminal | Kitty |
| Browser | Brave Origin Nightly |
| Files | Nautilus |
| PDF | Papers |
| Images | Loupe |
| Text | GNOME Text Editor |

```sh
scripts/applications launch terminal
scripts/applications check --personal
scripts/applications apply --personal
scripts/applications restore
```

The installer verifies executables and desktop entries before assigning defaults.
It updates the designated keys in `$XDG_CONFIG_HOME/mimeapps.list`, recording
original values in `$XDG_STATE_HOME/desktop-foundation/application-roles.json`.
Reruns retain those originals; restoration reverses the managed assignments.
Externally changed managed keys are reported as conflicts.
