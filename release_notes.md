Release Notes
=============

v0.1.0 (05/07/2026)
-------------------
- [f09aeee](https://github.com/thevickypedia/RuTorrent/commit/f09aeee171096bc994a22a3aed32b925373d70b5) chore: Release ``v0.1.0``
- [9fa65ed](https://github.com/thevickypedia/RuTorrent/commit/9fa65ed71af9853c1393271ff7f94ca6ebc6a6ab) docs: Update README.md
- [429592b](https://github.com/thevickypedia/RuTorrent/commit/429592b807ac1d282fb14a9acf473271ce20653d) fix: Handle an edge case for ``db`` insertion where QBitAPI may have multiple tags
- [7df0c9a](https://github.com/thevickypedia/RuTorrent/commit/7df0c9a0af9be6646b707561ed446d0e127e076b) perf: Simplify architecture for rsync target and tracker
- [8a7c1cb](https://github.com/thevickypedia/RuTorrent/commit/8a7c1cbc2ac7a71e9330753a34b957e90cea5c7b) perf: Remove batch mode for rsync
- [5e61707](https://github.com/thevickypedia/RuTorrent/commit/5e61707e419f7caa0b8eb520f0b520d0b95340db) feat: Include a new option to delete after copy for ``PUT /torrent`` endpoint
- [5417dfe](https://github.com/thevickypedia/RuTorrent/commit/5417dfe1208a546b47a8f0d22184698465f2c274) refactor: Rename QBitAPI references to honor appropriate naming convention
- [28ba682](https://github.com/thevickypedia/RuTorrent/commit/28ba682692fa2bb969321a3b3a23aa0900738dbd) docs: Update README.md
- [81468a0](https://github.com/thevickypedia/RuTorrent/commit/81468a07e8eba33d98ce79644a1c60d4a7a34d24) docs: Update README.md
- [35c3942](https://github.com/thevickypedia/RuTorrent/commit/35c39428fdc94af66d8665ebfdff4ad9dbbc47f0) perf: Simplify error handling for notification service
- [5420594](https://github.com/thevickypedia/RuTorrent/commit/5420594d8d9ab5629fd153a3af5eb2a51d82b58b) feat: Add a new feature to send telegram notifications
- [d28767d](https://github.com/thevickypedia/RuTorrent/commit/d28767d23f9e22b9c7f425520cafb584e914c80c) docs: Update README.md
- [16c3776](https://github.com/thevickypedia/RuTorrent/commit/16c3776958e51182f7c30b54bbda8f0d1f94eebe) feat: Include notifications for failed transfers
- [f598004](https://github.com/thevickypedia/RuTorrent/commit/f598004272a4ed98de3410cee37c2aa137ceb919) refactor: Remove ``Option`` on ``RsyncTarget`` to simplify code
- [1089eab](https://github.com/thevickypedia/RuTorrent/commit/1089eab9065208e31c712d88cd446de3be15c1e5) feat: Include notifications via NTFY
- [3e47a7b](https://github.com/thevickypedia/RuTorrent/commit/3e47a7bad546029da19344552a4e416df29aa54e) docs: Update docstrings and README.md
- [387cb44](https://github.com/thevickypedia/RuTorrent/commit/387cb44b610cb98ef450a66fb13962293e33967a) perf: Strip ``/`` suffix from QBitAPI url during startup
- [4689722](https://github.com/thevickypedia/RuTorrent/commit/468972220e2418681227d2830e99a2bd0844cd0c) refactor: Update swagger logic to a dedicated module
- [525364e](https://github.com/thevickypedia/RuTorrent/commit/525364ea7be99d4742bb0ebb4c18d6745dbb3479) feat: Include ``apikey`` based authentication mechanism
- [c4f8f1a](https://github.com/thevickypedia/RuTorrent/commit/c4f8f1a00b83fd11f081928e4cf31bec7ff32339) feat: Include a swagger UI endpoint
- [d19fd21](https://github.com/thevickypedia/RuTorrent/commit/d19fd211f67da890287a92c3d4466c260c392db4) chore: Update release notes for v0.0.4

v0.0.4 (05/05/2026)
-------------------
- [773bccd](https://github.com/thevickypedia/RuTorrent/commit/773bccd8155b8ef6fe5292841022fe2d510ec1b0) chore: Release ``v0.0.4``
- [3ef3686](https://github.com/thevickypedia/RuTorrent/commit/3ef368684a81bc7e22ec0ec00c1deb73e87db84a) feat: Include an option set the number of workers for ``actix`` API
- [76065b3](https://github.com/thevickypedia/RuTorrent/commit/76065b3979bf032514d3afcab32ae6bcf276beb2) feat: Add an option to set log level
- [fdc32c3](https://github.com/thevickypedia/RuTorrent/commit/fdc32c3b034f1ec0a8533e012481e41a19b33138) perf: Create a client during startup to validate initial auth
- [d32b1f3](https://github.com/thevickypedia/RuTorrent/commit/d32b1f36a9f2321a339e2b7985d14d5df8814549) perf: Avoid making network calls when local db and state is empty
- [bc21bca](https://github.com/thevickypedia/RuTorrent/commit/bc21bca36d5a5ef84c959a06d5b3fed4f43f0adc) perf: Replace time based re-auth mechanism with auth based login attempt
- [95bd967](https://github.com/thevickypedia/RuTorrent/commit/95bd9674ec4201f2f0915394e8bc0916428902ea) perf: Avoid creating a new client for every iteration in worker process
- [c470610](https://github.com/thevickypedia/RuTorrent/commit/c47061038bf37c09e2bfe40af151bba8bb935b32) feat: Add ``/status``, ``/health`` and ``/version`` API endpoints
- [b6305c9](https://github.com/thevickypedia/RuTorrent/commit/b6305c96398dd97a8870c2bb7f889f4954c5e48e) chore: Update application summary in README.md and project metadata
- [a1fc632](https://github.com/thevickypedia/RuTorrent/commit/a1fc63231c88c447519462107f69cb3f087f25db) docs: Update docstrings and README.md
- [1902956](https://github.com/thevickypedia/RuTorrent/commit/1902956166955c04e7361a2d29a046fdf12a20da) feat: Add ``savepath`` as an optional env var and override through ``PUT /torrent``
- [b459924](https://github.com/thevickypedia/RuTorrent/commit/b459924cb16fce0f318024896847de9eec19cb92) chore: Update release notes for v0.0.3

v0.0.3 (05/05/2026)
-------------------
- [f9c8989](https://github.com/thevickypedia/rutorrent/commit/f9c8989193ca90b05818f12ac82a396825ae39b6) chore: Release ``v0.0.3``
- [f4e745c](https://github.com/thevickypedia/rutorrent/commit/f4e745c2e48eab7148ba62e1dd02ee956405d342) chore: Update .gitignore
- [a99b504](https://github.com/thevickypedia/rutorrent/commit/a99b50447a5a63aef46336d281f9d3496794a97f) feat: Resolve case agnostic env vars
- [1300a2e](https://github.com/thevickypedia/rutorrent/commit/1300a2ec4c86860540f8cd3b6a57e175d2ef2138) feat: Avoid passing existing magnet links to QBitAPI
- [a916234](https://github.com/thevickypedia/rutorrent/commit/a916234030766c95a6186f850d50e6e4b314323d) chore: Update release notes for v0.0.2

v0.0.2 (05/04/2026)
-------------------
- [2bdc2f6](https://github.com/thevickypedia/rutorrent/commit/2bdc2f6ebfed819f8e012eee42332562265aa91b) chore: Release ``v0.0.2``
- [301dada](https://github.com/thevickypedia/rutorrent/commit/301dadac8fcb5c651e12bd2da675236fbbb1ae27) feat: Support dotenv files to load env vars
- [d78fc73](https://github.com/thevickypedia/rutorrent/commit/d78fc73e8aca6188a7fb62687f7f557e6f55e606) refactor: Avoid prompts during run-time for rsync
- [fd706c6](https://github.com/thevickypedia/rutorrent/commit/fd706c6bd2e8da4641be40f55349a08aa160e82a) feat: Include an option to set remote host values via env vars
- [4a4eb8b](https://github.com/thevickypedia/rutorrent/commit/4a4eb8b80ca3a8b3c2bdb1f125dfa48f908d3609) chore: Update release notes for v0.0.1

v0.0.1 (05/04/2026)
-------------------
- [73fbeb0](https://github.com/thevickypedia/rutorrent/commit/73fbeb082deac43d50579292ea97f0995f26bcde) chore: Release ``v0.0.1``
- [6364d20](https://github.com/thevickypedia/rutorrent/commit/6364d204bf9a9bd1fa4f32c474886e89fcd5c6d0) ci: Extend GHA to auto generate release notes
- [06f8df8](https://github.com/thevickypedia/rutorrent/commit/06f8df8b25459bc8acf92819599e6e5c97acd760) docs: Update docstrings and restrict release to run only for changes in project metadata
- [02fb894](https://github.com/thevickypedia/rutorrent/commit/02fb8941a0a3693aca16f87ce285ed46edcf7906) ci: Add a GHA workflow to build, test and upload artifact
- [8760de8](https://github.com/thevickypedia/rutorrent/commit/8760de8dc520006747f57b981cefa3788c9db234) refactor: Restructure code base and bump dependencies
- [27528fe](https://github.com/thevickypedia/rutorrent/commit/27528fe917e3cc56e621f383b11a913b74871b5f) feat: Allow ``PUT /torrent`` to function without rsync information
- [dd81f67](https://github.com/thevickypedia/rutorrent/commit/dd81f6761f6cd7b245e5fb13685c4e42bfaa8957) feat: Bring delete functionality back with much defined usage
- [8913b59](https://github.com/thevickypedia/rutorrent/commit/8913b59a7c1f5e2e4788530e5cd93633930fd1b6) refactor: Improve magnet -> rsync receivers mapping
- [48240a0](https://github.com/thevickypedia/rutorrent/commit/48240a0549f15856f79e936b53bd0376d90d802b) refactor: Implement ``UUID`` based name -> hash resolution
- [6050892](https://github.com/thevickypedia/rutorrent/commit/6050892c7dff948d87736724745179e713d61cd8) refactor: Improve code quality and readability
- [ca018ef](https://github.com/thevickypedia/rutorrent/commit/ca018efbb1a3dfb144e66e9cc33ba8f6d080fc93) feat: Replace in-house scp based rsync with battle-tested CLI tool
- [3c757a7](https://github.com/thevickypedia/rutorrent/commit/3c757a74ebdea8ad7d8e8a4928dc74cdc6486742) fix: Fix hash:name resolution issue stalling rsync
- [85fe2cd](https://github.com/thevickypedia/rutorrent/commit/85fe2cd507dc61f3dd94aa2c079789b19a762a65) feat: Include URL specific SSH host, username and password in PUT ``/torrent``
- [d2a33fb](https://github.com/thevickypedia/rutorrent/commit/d2a33fb764e90fcf9a588d7a1aa50d79a8c72ce0) feat: Create an in-house rsync like ssh transactor
- [4e3f6ce](https://github.com/thevickypedia/rutorrent/commit/4e3f6ced2df1feaa13031a837bea40cb7a3a0444) refactor: Simplify client creation and login workflows
- [5a6746f](https://github.com/thevickypedia/rutorrent/commit/5a6746fd4bcf54a6b298a3007d7736780f6a1b80) feat: Implement logger
- [b1cbc3a](https://github.com/thevickypedia/rutorrent/commit/b1cbc3ae02d071ef7790c7bff08ce1283038a66f) refactor: Move all config values to settings.rs and remove dead code
- [6499e35](https://github.com/thevickypedia/rutorrent/commit/6499e3550f476620a4f3e403810c02059a7c61bb) fix: Use torrent name as identifier to resolve the hash for DELETE endpoint
- [2fec3cc](https://github.com/thevickypedia/rutorrent/commit/2fec3cc7fa850b41faefe825ae50f74afb7e715f) feat: Implement full architectured API
- [6d665b7](https://github.com/thevickypedia/rutorrent/commit/6d665b761f910da27660d6197cad6a38d673036c) feat: Move constants to env vars
- [09e659d](https://github.com/thevickypedia/rutorrent/commit/09e659dc156b4cdac09d3d838701c15fe8818807) feat: Include a feature to track progress with multiple URLs
- [93bb5ca](https://github.com/thevickypedia/rutorrent/commit/93bb5ca731c4b1eee3b953dfafa302221410154a) feat: Create a base project to download magnet URLs
- [eba5d9d](https://github.com/thevickypedia/rutorrent/commit/eba5d9d23636489b5b010be83b279dab71f4634e) init: Add a hello-world cargo project
- [ba1203f](https://github.com/thevickypedia/rutorrent/commit/ba1203fc5ce21dd03f0d608333672d4dc1af64ad) init: Add project basics
