# Fonts

The desktop UI uses Google Sans Flex, with Google Sans Code for monospaced labels. Kitty
uses GoogleSansCode Nerd Font Mono; Fish and Fastfetch inherit the terminal font.

Bundled sources:

- Google Sans Flex from `google/fonts`, revision
  `6157dfaa640ca5738b4d1a7070fe389c3036d7fc`.

- Google Sans and Google Sans Code from `google/fonts`, revision
  `9710da1eacb3be272583c3224dcb70f9da6eadbb`.
- GoogleSansCode Nerd Font Mono from `ryanoasis/nerd-fonts` v3.5.1.
- Download URLs and SHA-256 checksums are listed in `sources.json`.

Each family includes its upstream SIL Open Font License. Google trademark notices
are included with the font files.

Deployment links this directory into `$XDG_DATA_HOME/fonts/desktop-foundation`
and refreshes fontconfig. Open new application windows to pick up font changes.

GTK and the desktop sans-serif/system-ui defaults use Google Sans Flex.
The installer journals the GNOME interface font preference and the fontconfig
override for restoration through the existing uninstall path. Applications with
an explicit custom font keep that setting; code and terminal fonts stay monospaced.
