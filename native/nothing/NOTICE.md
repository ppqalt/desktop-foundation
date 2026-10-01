# Attribution and source

This helper is licensed under GNU AGPL version 3 (LICENSE).
The model database in src/upstream/models.rs is from Daan Hessen's earctl:
https://github.com/DaanHessen/earctl
commit e31159c7dffe10765cecedd8da3d4a2ad2f93985.
The framing and setting codecs in src/protocol.rs and src/settings.rs adapt
that project's protocol.rs/service.rs. Changes: bounded stream recovery,
16-bit payload length, strict response validation, correlated readback,
missing batteries represented as null, safe serial parsing.

Protocol research also used radiance-project/ear-web:
https://github.com/radiance-project/ear-web
commit 440cd36c2a3f3db05f116ac634c3a7a3c2b7ebd5.
Credit to its contributors, including RapidZapper, Bendix, Lisra-git and
MemerGamer, acknowledged by earctl. No web UI, icons or assets are included.

The helper's complete corresponding source and locked dependencies are in
this directory. It communicates over local stdio; it runs no network server.
Distribute source and this notice/license with any compiled helper.

Additional protocol research, retrieved 2026-10-01 from the deployed ear (web):
https://earweb.bttl.xyz/js/bluetooth_socket.js
SHA256 c7f2aa06a5d6b832951ca4d3cc5321da637dc2e1485305f3fbc5d92371d642e6
https://earweb.bttl.xyz/js/ear_config_file.json
SHA256 34f1aa6a7c2b4d55ec8ca8214b61d42460a15eff4eb1cf5ffa8ee4b3552e8085
The deployed version is newer than its public Git checkout. Its command/response
pairs and B173 feature flags informed the native implementation of multipoint,
Audiodo profile enable, quality preference, spatial-state query, Super Mic,
call transparency and fit-test result. No deployed UI, images or brand assets
are included. Verified against local B173 firmware 1.0.1.69.
