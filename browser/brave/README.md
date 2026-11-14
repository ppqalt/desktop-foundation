# Brave Origin configuration

Standard uses stable `brave-origin-bin`; legacy Personal retains Nightly. The
installer seeds bundled settings once for a fresh selected profile and preserves
existing profiles. Use `scripts/post-install brave` for explicit later setup. The helper discovers the browser root and selected
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

Use `--channel stable` or `--channel nightly` to select explicitly.
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

## Nightly to Stable

`scripts/brave-config migrate` copies the closed default Nightly root to an absent
Stable root. The original stays untouched. Newer Nightly versions, open browsers,
changed source files, insufficient space or an existing destination prevent
replacement/merging. `--user-data-dir PATH` selects a custom source explicitly.
The migration journal is in the user's DF Brave state directory; no profile
contents or keys are logged. Standard leaves MIME defaults unchanged while
migration is pending. Do not force a downgrade of a newer Nightly profile.

Source contracts: [CachyOS package](https://github.com/CachyOS/cachyos-aur-derived/tree/master/brave-origin-bin),
[upstream profile discovery](https://github.com/brave/brave-core/blob/master/chromium_src/chrome/common/chrome_paths_linux.cc),
and [Linux policy directory](https://github.com/brave/brave-core/blob/master/app/brave_main_delegate.cc).
