# Application roles and defaults

`config/application-roles.json` is authoritative for package, executable,
desktop entry and MIME assignments. Niri terminal/files/browser bindings call
`scripts/applications launch ROLE`, which uses the selected installation profile.
Core remains usable without Brave or other personal applications.

| Role | Core | Full |
| --- | --- | --- |
| Terminal | Kitty | Kitty |
| Browser, HTML, HTTP/HTTPS/about/unknown | Firefox | Brave Origin Nightly |
| Directories | Nautilus | Nautilus |
| PDF | Papers | Papers |
| PNG/JPEG | Loupe | Loupe |
| Plain text | GNOME Text Editor | GNOME Text Editor |

Before writing, the helper verifies each executable and its valid desktop entry,
including that Exec resolves to the declared executable. It updates only the
owned keys in `$XDG_CONFIG_HOME/mimeapps.list`'s Default Applications section,
leaving other keys, sections and comments intact. Original values and pending
publication are recorded in `$XDG_STATE_HOME/desktop-foundation/application-roles.json`.

Afterward it queries every assigned MIME and `xdg-settings get default-web-browser`.
Desktop-specific MIME overrides can defeat a generic file: verification reports
that conflict rather than rewriting another desktop's file blindly.

Old v0.10/v0.11 MIME journals migrate only defaults actually changed between the
original and managed file. Unrelated intervening edits are accepted. The old
backup/journal is retained as migration evidence, not restored wholesale.

Rerunning preserves original values. A changed owned key is a conflict. Restore
updates/removes only owned keys; unrelated later MIME edits survive. A new empty
file can be removed after restore. Pending writes are recoverable by rerunning.

```sh
scripts/applications check --personal
scripts/applications apply --personal
scripts/applications restore
```

These are focused helpers; normal installation calls them automatically. Core
inspection accepts an already-valid personal browser role when full was selected.
Installing core explicitly selects core defaults; use install-all to retain full
personal assignments. Unrelated apps are never removed.
