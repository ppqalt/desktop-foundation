# Browser wallpaper colors

Desktop Foundation derives a dark browser tint from the wallpaper's semantic
`elevated` color. Brave's native `BrowserThemeColor` policy generates its browser
palette from that tint. This follows wallpaper hue without using a bright accent
as the browser background. The browser determines its individual toolbar and tab
colors, so this mode does not reproduce every color of the unpacked graphite theme.

## Setup

Build the backend, then install the root-owned writer for your desktop account:

```sh
scripts/build-backend
sudo scripts/browser-policy-setup install
```

When running directly as root, specify `--user USERNAME`. The installed helper
accepts only `apply #RRGGBB`, `remove` and read-only `status`. Its polkit action is
limited to this helper and the named local, active desktop account. It cannot
accept a destination path, arbitrary policy keys, source files or browser data.
No setuid executable is used.

The policy file is:

```text
/etc/brave/policies/managed/70-desktop-foundation-theme.json
```

Directories remain root-owned and unwritable by other users. The policy contains
only `BrowserThemeColor`; unrelated managed policies are preserved. This Linux
policy directory is shared by Brave channels and applies to their profiles,
including Brave Origin Nightly. A managed theme overrides browser theme choices
while installed. Removing it lets Brave restore the previous selected theme.

## Refresh

`scripts/wallpaper-set` publishes the palette, updates the narrow policy and sends
Brave's native `--refresh-platform-policy` request to an already running Origin
Nightly instance. The request opens no window. When the browser is closed, only
the policy is updated; Brave reads it on its next start. No debugging server,
resident helper or browser profile edit is involved.

Origin Nightly uses `brave-origin-nightly`, its real executable under
`/opt/brave.com/brave-origin-nightly/`, and the user-data directory
`~/.config/BraveSoftware/Brave-Origin-Nightly`. Refresh detection checks process
ownership, executable and selected user-data directory, rather than matching the
launcher name against Chromium's `brave` process name. Another custom user-data
directory is left alone.

`BrowserThemeColor` does not select dark/light mode. Keep Brave's native appearance
setting on **Dark**, or **Same as system** with the desktop's dark preference.
Desktop Foundation does not rewrite an active profile to change that choice.
An installed extension theme does not need to be manually removed: the policy
takes precedence, preserving its prior selection for restoration.

Check `brave://policy` for `BrowserThemeColor`, its value and **OK** status. A
native policy refresh can be requested directly through **Reload policies** on
that page. Without the installed policy writer, the generated unpacked theme
remains available through **Load unpacked**; approved DevTools refresh is an
optional separate adapter.

## Restore

```sh
sudo scripts/browser-policy-setup restore
```

The transaction restores its original policy, helper and polkit files after
checking their ownership and saved contents. Externally modified files are not
overwritten. Only directories created by the setup and still empty are removed.
Brave sees policy removal at its next native refresh or start. Wallpaper rollback
uses the retained semantic palette and the same policy update path.

## Why the earlier adapter needed manual refresh

The unpacked theme's manifest changed correctly, but Chromium does not watch the
theme directory and reapply file changes. The earlier automatic route required
Brave's separately enabled and approved DevTools connection; when that adapter
was disabled, preparing the manifest could not update the running browser.

The policy route uses Chromium's supported dynamic policy refresh. It avoids
iNiR's writable policy directory and active `Preferences` edits. It also detects
Origin Nightly explicitly rather than assuming standard Brave's executable and
profile paths.

References: [BrowserThemeColor](https://chromeenterprise.google/policies/browser-theme-color/),
[Chromium theme policy handling](https://github.com/chromium/chromium/blob/main/chrome/browser/themes/theme_service.cc),
[native policy refresh](https://github.com/chromium/chromium/blob/main/chrome/browser/ui/startup/startup_browser_creator.cc),
[Brave policy directory](https://github.com/brave/brave-core/blob/master/app/brave_main_delegate.cc),
[iNiR comparison](https://github.com/snowarch/iNiR/blob/main/scripts/colors/apply-chrome-theme.sh).
