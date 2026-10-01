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
