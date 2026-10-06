# Brave Origin Nightly configuration

Apply the bundled Origin Nightly settings through `scripts/post-install brave`
after full installation. The helper discovers the browser root and selected
profile; use `--user-data-dir` and `--profile` to choose explicitly.

## Files

- `allowlist.json`: permitted preference keys and types.
- `preferences.json`: browser presentation, tab, autocomplete, language and
  privacy settings.
- `extensions.json`: extension IDs and Chrome Web Store links.
- `search.json`: Brave Search configuration.
- `flags.conf`: package-supported launch flags.
- `config.py`: export, application and rollback.

## Apply and restore

Close Brave normally before applying settings:

```sh
scripts/post-install brave
scripts/brave-config apply --refresh
scripts/brave-config restore
```

The initial seed runs once per selected profile. Rerunning installation keeps
later interactive changes; `--refresh` reapplies the bundled allowlisted values.
Restore uses the original values from the configuration journal and refuses
externally changed fields. `scripts/brave-config export` updates the tracked
allowlisted settings from the selected profile.

## Extensions and theme

Dark Reader, SponsorBlock, FrankerFaceZ, Return YouTube Dislike and ChatGPT are
listed for installation through their Chrome Web Store pages. Run
`scripts/brave-config extensions` to show the links.

The generated wallpaper theme is separate from these settings. Load its stable
folder through **Load unpacked**, or configure approved DevTools refresh. See
[theme controls](../../theme/README.md).
