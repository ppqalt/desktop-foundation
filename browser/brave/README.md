# Personal Brave Origin Nightly configuration

This is an allowlisted export from the live Origin Nightly `Default` profile,
inspected on 2026-10-02. The helper discovers the installed Origin Nightly wrapper,
active processes/user-data arguments and Local State's selected profile. It never
uses a stable/Beta/ordinary Nightly profile as a substitute. Multiple roots or
profiles require explicit `--user-data-dir` / `--profile` selection.

## Tracked configuration

- `allowlist.json`: exact permitted dotted keys with narrow value types.
- `preferences.json`: 33 present personal values only, no whole preference tree.
- `policies.json`: deliberately empty. No mandatory or recommended system policies
  are deployed, and no settings are locked into a managed-browser UI.
- `extensions.json`: five public extension IDs, names and Chrome Web Store links.
- `search.json`: stock Brave Search, left to Brave's native default.
- `flags.conf`: tracked package-supported launch flags, linked with a backup.
- `config.py`: discovery, deterministic export, seed/refresh and journal rollback.

Exported categories: theme/color/frame, bookmark-button/bar presentation, tab
hover/middle-click/group behavior, vertical-tab panel presentation, autocomplete,
spellcheck, PWA/forward buttons, HTTPS-only/upgrades, WebRTC public-interface
handling, first-party sets, language/translation, Shields presentation, media
routing and history retention duration. The latest autocomplete change is ON.

No explicit startup/homepage, download, sidebar visibility or hardware
acceleration override was present. Their native defaults remain. The hardware
"previous mode" field is runtime state, not an intentional configuration value,
and is excluded. Sidebar last-used item, extension toolbar runtime metadata and
window geometry are excluded. No explicit launch flags or experimental flags were
present; native Ozone selection remains the installed browser's default. To choose
Wayland explicitly later, add `--ozone-platform=wayland` to `flags.conf`.

Search suggestions are off. Protected search-provider metadata, hashes, IDs and
Sync GUIDs are not copied; current Brave Search is already the native default.
Custom search engines, startup URLs and site-specific content exceptions must be
configured interactively if needed; they can contain private addresses/data.

## Installation and future updates

The personal `scripts/install` calls `scripts/brave-config apply`; `--core` does
not. A closed fresh browser receives the seed before its first launch. Existing
unrelated state stays intact. Configuration is seeded once per selected profile;
rerunning does not overwrite subsequent interactive changes. An explicit
`--refresh` reapplies only allowlisted fields, retaining original values for undo.

If Brave is open, the helper writes nothing and reports:

```bash
# Close Brave normally, then:
./scripts/post-install brave
```

This also prints the extension installation links. Do not launch Brave concurrently
with apply/restore. Stale SingletonLock is handled by Brave itself, never deleted
by this helper. Export is read-only toward the live profile and works while it is
open (use after saving the browser's settings):

```bash
./scripts/brave-config export
./scripts/brave-config apply --refresh  # closed browser; explicit reset of seeds
./scripts/brave-config restore          # closed browser; refuses modified fields
```

The state journal stores only original allowlisted values and flags backups below
XDG state, never in Git. Preference writes are atomic and private. Secure
Preferences/integrity hashes are never modified. Check defaults with:
`xdg-settings get default-web-browser` → `brave-origin-nightly.desktop`.
The existing personal installer owns the MIME defaults and Super+W integration.

## Extensions

Dark Reader, SponsorBlock, FrankerFaceZ, Return YouTube Dislike and ChatGPT are
recorded with their current IDs. Install through the linked official Chrome Web
Store pages. Automatic external-extension paths vary with Chromium branding and
Brave Origin packaging; no unverified user-directory trick, copied extension
folders or force-install policy is introduced. These five manual installs are the
remaining extension step. Their settings, login state and permissions are not
restored. They remain removable/user-controlled.

Sources: [Brave's supported installation flow](https://support.brave.com/hc/en-us/articles/360017909112-How-can-I-add-extensions-to-Brave),
[Chromium preferences and policies](https://www.chromium.org/administrators/configuring-other-preferences/),
[external extension installation](https://developer.chrome.com/docs/extensions/how-to/distribute/install-extensions),
[branding-dependent providers](https://github.com/chromium/chromium/blob/main/chrome/browser/extensions/external_provider_impl.cc),
[Origin data-directory discovery](https://github.com/brave/brave-core/blob/master/chromium_src/chrome/common/chrome_paths_linux.cc).

## Privacy and validation

Never export cookies/passwords/tokens, account/Sync identifiers, history records,
open tabs/windows, autofill/payments, site exceptions, extension runtime databases,
IndexedDB/localStorage, service workers or cache. Profile-directory and raw-file
patterns are ignored by Git. All seeds are checked against typed allowlists on
apply; unknown keys are rejected. Extension names and IDs are constrained, links
are reconstructed from IDs. Export JSON is sorted and contains no timestamps.

Six isolated regression tests cover secrets exclusion, wrong-type/unknown keys,
fresh seeding, existing private-state preservation, repeat/change behavior,
open-browser refusal, flags backup/rollback and unsafe profile paths. The real
headless browser initialization attempt timed out; it does not count as proof of
first-launch UI behavior. The live browser was not modified or forcibly closed.
First graphical launch on a fresh installation remains the acceptance check.
