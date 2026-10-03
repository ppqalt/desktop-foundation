# v0.12 physical acceptance checklist

This checklist does not instruct wiping the reference machine. It applies when a
fresh OS or lucky38 test is available. Automated isolated HOME tests and reference
live checks have run; physical fresh-install/lucky38 acceptance has not.

1. Start from your normal Arch-family install with working graphics, networking,
   sudo, Git and Python. Use a compatible Niri/Matrix tuigreet package; inspect
   actual capability failures instead of assuming a version is enough.
2. Clone the tag into a permanent checkout. Inspect core/full dry-run.
3. Run core (choose --no-greeter for an existing login manager). Then optionally
   full. Confirm configured sources provide ChatGPT and multilib Steam; no
   third-party repositories are added automatically.
4. Log into Niri with password/keyring unlock. Run doctor and selected --check.
5. Before moving the pointer, open launcher. Check clipboard/power/Bluetooth,
   keyboard and wheel selection, Escape cleanup; do not execute shutdown in a test.
6. Finnish punctuation; one-column 1/3, 1/2, 2/3 centering; second-column scrolling;
   manual center, floating/fullscreen, focus border/depth and readable transparency.
7. Launch terminal/files/browser. Query directory, HTTP/HTTPS, PDF, PNG/JPEG and
   plain-text handlers. Open a harmless sample of each type.
8. Print and region screenshots copy; area starts empty and release completes;
   no Niri screenshot file is saved. Notification disappears; volume moves by 3%
   without excessive sound. Restore volume after a test.
9. First Spotify login/minute/normal quit/reopen, playback and Marketplace.
   Native reload after wallpaper change. ChatGPT launch/login is your action.
   Stock Steam launches; Millennium/Material is outside v0.12 acceptance.
10. Apply a controlled alternate wallpaper, verify semantic surfaces/accents,
    cached no-op and rollback to original. Brave native reload is manual;
    debugging remains off. Verify one swaybg and no resident Matugen.
11. Rerun installer. Verify original backups retained and unrelated files/defaults
    unchanged. Model an owned external edit: it must stop, never bulldoze.
12. Inspect AppArmor and failed services after any required boot activation. Use
    supported native bootloader configuration; do not convert dracut/mkinitcpio.

Before any separate future OS reinstall, verify your own off-machine backups.
The historical [first candidate procedure](FRESH_INSTALL_TEST.md) documents the
old tag only, not the v0.12 installation commands.
