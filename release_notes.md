Release Notes
=============

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
