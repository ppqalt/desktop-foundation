# Fonts

The shell uses Google Sans, with Google Sans Code for monospaced labels. Kitty
uses GoogleSansCode Nerd Font Mono; Fish and Fastfetch inherit the terminal font.

Bundled sources:

- Google Sans and Google Sans Code from `google/fonts`, revision
  `9710da1eacb3be272583c3224dcb70f9da6eadbb`.
- GoogleSansCode Nerd Font Mono from `ryanoasis/nerd-fonts` v3.5.1.
- Download URLs and SHA-256 checksums are listed in `sources.json`.

Each family includes its upstream SIL Open Font License. Google trademark notices
are included with the font files.

Deployment links this directory into `$XDG_DATA_HOME/fonts/desktop-foundation`
and refreshes fontconfig. Open new application windows to pick up font changes.
