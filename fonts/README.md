# Portable Google typography

The shared shell, Niri notifications and region selector use **Google Sans**.
Shell monospaced text uses **Google Sans Code**. Kitty uses **GoogleSansCode Nerd
Font Mono**, including regular, bold, italic and bold italic. Fish and Fastfetch
inherit Kitty's font; neither needs a separate font setting.

Files are vendored here so deployment works offline on Tops and lucky38:

- Google Sans and Google Sans Code: official `google/fonts` repository, pinned to
  commit `9710da1eacb3be272583c3224dcb70f9da6eadbb`.
- Nerd Font Mono: official `ryanoasis/nerd-fonts` release **v3.5.1**. Its archive
  SHA-256 was checked against the digest published by the GitHub release API.
- Exact download URLs and per-file SHA-256 checksums are recorded in sources.json.
- Each family includes its upstream SIL Open Font License; Google trademark
  notices are retained alongside the official font files.

No upstream font sources are built or modified. Google Sans/Code variable fonts
provide their native weight ranges. The Nerd Font build is static and monospace.

Deployment backs up an existing user `fonts/desktop-foundation` path, links this
folder beneath `${XDG_DATA_HOME:-~/.local/share}/fonts/`, and refreshes fontconfig.
Rollback restores that path and refreshes fontconfig. Other user/system fonts
are untouched. Applications should be restarted to pick up the new font choices;
existing terminals are not forcibly closed.

This changes the foundation's shell and terminal typography. It does not force
fonts into unrelated applications or modify global GTK/Qt preferences.
