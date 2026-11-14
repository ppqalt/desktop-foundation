# Installer audit resolutions

Scope: the ten findings from local `43e1967`, plus normal EAC integration. This is
source implementation and focused fixture evidence, not fresh-OS/firmware acceptance.

| Finding | Resolution |
| --- | --- |
| 1. First bootstrap changes runtime packages/services before validation | Public grammar, owner and managed-link checks precede bootstrap. First compiler bootstrap uses `--build-only`: rustup/gcc/pkgconf/sqlite/dav1d/wayland/git, no desktop application stack or service startup. Native validation precedes profile provisioning. Inspection never bootstraps. Compiler prerequisites may still be installed before deeper native validation is possible. |
| 2. Minimal/Standard application shortcuts require missing bundled apps | Standard includes GNOME utilities and stable Origin. Missing browser/files roles fall back to an existing XDG default through gio; no MIME changes for that fallback. With no browser/file manager installed, a clear missing-application error remains appropriate. |
| 3. Everything reports success despite first-use add-ons pending | Completion explicitly means desktop installation. Add-on results and first-use limitations remain visible; no forced Spotify login or game installation. |
| 4. Replay autostarts just because the tool exists | Autostart requires recording in the saved installation selection, plus the existing recording preference. Standard/Personal retain five-minute replay behavior; Minimal needs the extra unless the user explicitly enabled replay autostart in their own config. Older installations without a selection retain compatibility. |
| 5. Fresh default is a photo | New default mode is `none`: no swaybg renderer. A neutral swatch supplies palette/overview metadata, not a displayed wallpaper or new photo in the collection. Existing runtime selections remain authoritative. Doctor understands the absent renderer. |
| 6. Missing network/update integration dependencies | Common update controls have explicit cachy-update dependency (provides arch-update too). Hardware services start after validation. NetworkManager is not enabled over active networkd/ConnMan; the installer reports that existing networking remains external. |
| 7. Standard always includes paru | Intentionally retained at the owner's explicit request. Standard provisions it and validates it in no-package/check mode. Stable Origin itself requires a configured signed pacman repository; it never falls back to AUR. |
| 8. Missing/stale inspection backend and overstated verification | Source-digest mismatch rejects stale inspection without building/writing. A missing compiler backend is an explicit prerequisite, not a successful inspection. Doctor covers Standard, recording and firmware; source/fixture checks do not certify hardware, login, add-ons or recovery boots. |
| 9. Alias-only display-manager detection | Shared detector also checks active/enabled known managers, preferring active ones. The privileged greetd writer repeats this check. Existing managers and their PAM configurations stay intact. Custom unknown managers still warrant explicit `--no-greeter`. |
| 10. Ambiguous options | `--everything` aliases legacy Personal; `--core` aliases Minimal. Help/docs clarify that `--no-packages` still builds, deploys and configures selected services. `--dev` remains a bootstrap development option, not an installation layer. |

EAC preparation runs after desktop deployment. Only durable safe firmware/boot
checkpoints produce a pending result; unexpected package/signature/certificate
errors remain failures. The saved selection is retained when desktop installation
succeeds with boot pending. Root EAC installation status and optional recovery
boot acceptance are separate, so an unperformed physical test cannot become a
false readiness claim.

Stable Origin is selected by Standard from `cachyos/brave-origin-bin`; legacy
Personal retains Nightly. Standard supports `--extra brave-nightly` for a parallel
optional install without changing its primary browser.

Stable Origin migration preserves both channels: independent copy, version and
busy checks, byte verification, no-replace publication, original root retained.
MIME migration touches only eligible browser defaults and keeps rollback originals;
foreign application defaults remain unchanged. A newer Nightly profile remains
pending rather than being forced into an older Stable browser.
