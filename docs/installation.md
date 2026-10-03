# v0.12 installation and convergence

There are two layers, not a collection of application-specific top-level commands:

| Command | Purpose |
| --- | --- |
| `scripts/install-core` | Complete portable Niri desktop, terminal and usable default apps |
| `scripts/install-all` / `scripts/install` | Reuse core, then provision the personal applications |
| `--check` | Read-only doctor/ownership/default inspection |
| `--dry-run` / `--plan` | Read-only package/ownership/deployment plan |
| `--no-packages` | Rebuild/deploy using installed dependencies; verify personal tools |
| `--no-greeter` | Retain an existing display manager or deliberate TTY login |
| `--hardware-profile NAME` | Select a tracked hardware profile; default is portable |
| `--clean-boot` | Explicit separately journaled GRUB/mkinitcpio cleanup |

`install --core`, `--personal` and `--profile` remain compatible. Internal
`install_engine.py` separates planning, core and personal convergence and reuses
bootstrap, deployment, preferences and application helpers. It is not a new
background framework.

## Prerequisites and fresh installation

A reasonably clean Arch/CachyOS/EndeavourOS system must already have a normal user,
sudo, network, working graphics, Git and Python. Run as that user, not root.
Clone v0.12 into a permanent directory, inspect `install-core --dry-run`, then run
`install-core`. Select Niri on the next login. Add `install-all` if desired.

CachyOS is the tested reference. Native Niri parser validation must accept blur,
background effects and the generated KDL; stock versions on other distros may
not. Matrix tuigreet is likewise capability-checked before authentication-file
changes. No external repository is added to solve incompatibility.

Core uses Finnish input, the existing graphite/window policy and strict native
single-column centering. No monitor, GPU, hostname or disk name is required.
Override hardware in `profiles/NAME/niri.kdl`; input remains in `config/input.lua`.

Full provisions Brave (configured repository or reviewed AUR), Spotify tools,
ChatGPT (verified signed CachyOS package) and stock Steam (multilib). Read
[personal applications](personal-apps.md) for availability/login boundaries.
Millennium/Material is not included in the v0.12 installation promise.

## Existing machines and upgrades from v0.10/v0.11

Keep the existing checkout and `$XDG_STATE_HOME/desktop-foundation` backups.
Fetch and inspect the new tag; do not delete the installation or transplant its
journal from another machine. Resolve uncommitted work without resetting it.

1. Run the selected installer's dry-run. Externally replaced owned links or MIME
   keys report conflicts before package/system mutations.
2. Use `--no-greeter` when another display manager owns login. No automatic
   disabling/force-replacement of it is performed.
3. Run the installer with packages enabled when dependencies change. Missing
   packages use full `pacman -Syu --needed`; declared compatible package providers
   satisfy core dependencies. AUR builds remain unprivileged and reviewed.
4. Core deploys staged/validated links with original backups. Kitty, Fish and
   Fastfetch directories are replaced with managed configurations and their old
   directories retained; this is a deliberate replacement, not a merge.
5. The old MIME whole-file journal migrates only keys the old installer actually
   changed. Later unrelated associations/comments survive. Explicit new roles
   repair wrong directory/browser choices. Original role values remain recoverable.
6. Current runtime wallpaper and semantic colors are retained when templates are
   re-rendered during deployment. No account data is imported.
7. Already-pinned tools are reused. Known older Spicetify 2.45.1 upgrades to the
   checksum-pinned 2.45.3 with its tool backup. Unknown versions/source conflicts
   are refused. The managed Spotify launcher completes first-login patching.

Reruns do not accumulate original-config backups, reset accounts or spawn duplicate
services. A deliberate deploy may create a new runtime revision to propagate
changed templates; ordinary unchanged wallpaper applications are no-ops.

Package installation can finish before a later conflict is discovered; packages
remain installed. Each protected mutation is journaled, not claimed to be a
single atomic whole-system transaction. Fix a reported conflict and rerun;
never delete a journal to suppress an error.

## System and boot safety

Bluetooth/AppArmor services are enabled. A kernel already enforcing AppArmor does
not get boot files regenerated. If activation is required, the existing supported
GRUB fragment helper preserves LSM arguments. Other bootloaders need native manual
configuration; dracut and mkinitcpio are never converted into each other.

Greetd/PAM files have protected backups; installation never restarts the active
display manager. The validated Matrix/keyring path is retained. A conflicting
manager stops this step and requires `--no-greeter` or an explicit manual choice.
The prior tty1/tty8 non-Plymouth handoff fix remains unchanged.

`--clean-boot` remains explicit and only supports an existing compatible
GRUB/mkinitcpio setup. Saved boot transactions are reviewed/resumed/restored with
their own helpers, rather than silently replacing the original recovery snapshot.
See [AppArmor](APPARMOR.md), [boot setup](boot-security-proposal.md) and
[boot optimizations](boot-optimization.md). No boot-affecting change is needed for
this v0.12 upgrade on an already-valid reference installation.

## Appearance and acceptance

Core retains supported GTK dark preference/adw-gtk3-dark and Google Sans.
Wallpaper changes map to the native discrete GNOME accent enum where supported;
libadwaita/portal behavior determines which apps honor it. Neutral surfaces are
not overridden with CSS. Niri notifications remain the existing managed Mako
service, top-center with bounded timeout; Quickshell provides the volume OSD.

Niri screenshots remain clipboard-only; Hyprland's secondary backend saves and
copies. Application launcher uses desktop entries, not Fuzzel or polling.

Run `scripts/doctor` (or `--personal`) and the
[fresh-login checklist](fresh-install-v012.md). Physical login, graphics, earbud
hardware and application account actions cannot be certified by a dry-run.

## Rollback

`uninstall` restores owned config/preference paths while leaving packages and
account/application data installed. `uninstall --greeter` also restores greetd/PAM
files. `spotify-setup restore` handles its optional links/config/tool upgrade;
Brave preferences were not changed by the v0.12 installer. Any older opt-in
preference seeding has its own `brave-config restore` journal.

MIME restore changes only recorded Default Applications keys. External changes
to an owned key/path cause a refusal; unrelated later edits are preserved.
For startup problems, authenticate on another VT and use the documented helper
from the same checkout. Never restart the display manager to repair an active chat.
