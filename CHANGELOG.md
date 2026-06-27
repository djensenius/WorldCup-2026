# Changelog

## [0.3.0](https://github.com/djensenius/WorldCup-2026/compare/v0.2.4...v0.3.0) (2026-06-27)


### Features

* show match venue location on the Matches list ([#18](https://github.com/djensenius/WorldCup-2026/issues/18)) ([a555cf6](https://github.com/djensenius/WorldCup-2026/commit/a555cf63edf7ef0d6ce92e569c5070e59b9f4740))


### Bug Fixes

* resolve config under XDG/~/.config and default flags off ([#19](https://github.com/djensenius/WorldCup-2026/issues/19)) ([4c20aa4](https://github.com/djensenius/WorldCup-2026/commit/4c20aa4fc8203da07f5a28baef1dfdd6d987f911))

## [0.2.4](https://github.com/djensenius/WorldCup-2026/compare/v0.2.3...v0.2.4) (2026-06-27)


### Bug Fixes

* add screenshots and an advanced config example to the README ([#17](https://github.com/djensenius/WorldCup-2026/issues/17)) ([abba082](https://github.com/djensenius/WorldCup-2026/commit/abba0825c3b2b4eb610783ad4a51e36ab6437fff))
* sync workspace lockfile during publish so crates.io release succeeds ([#15](https://github.com/djensenius/WorldCup-2026/issues/15)) ([1ae429c](https://github.com/djensenius/WorldCup-2026/commit/1ae429ccbfffadd0fd473e4dbdd7354b0a640164))

## [0.2.3](https://github.com/djensenius/WorldCup-2026/compare/v0.2.2...v0.2.3) (2026-06-26)


### Bug Fixes

* align Live card flags, top-anchor schedule, document config ([#13](https://github.com/djensenius/WorldCup-2026/issues/13)) ([4b10ac5](https://github.com/djensenius/WorldCup-2026/commit/4b10ac5db963d0ecde5ad7516bcd8f75a75d6734))

## [0.2.2](https://github.com/djensenius/WorldCup-2026/compare/v0.2.1...v0.2.2) (2026-06-26)


### Bug Fixes

* refresh lockfile before publishing crates ([7f92d5b](https://github.com/djensenius/WorldCup-2026/commit/7f92d5b12c4c6d706c001efdb09e88a7c56646c7))
* support tmux wezterm flags and cli ui overrides ([#12](https://github.com/djensenius/WorldCup-2026/issues/12)) ([1b8ef65](https://github.com/djensenius/WorldCup-2026/commit/1b8ef65a03831ddf41996d3d1cf25744fb0f1eb5))
* use crates api for publish checks ([d1f30cc](https://github.com/djensenius/WorldCup-2026/commit/d1f30cc3820da6235b661e296ae39590943fa97c))

## [0.2.1](https://github.com/djensenius/WorldCup-2026/compare/v0.2.0...v0.2.1) (2026-06-16)


### Bug Fixes

* prepare public crate release ([8e8e48f](https://github.com/djensenius/WorldCup-2026/commit/8e8e48fb9850d2782b4bbc9ed66b1befe28c2112))
* remove unmaintained paste dependency ([#7](https://github.com/djensenius/WorldCup-2026/issues/7)) ([846de34](https://github.com/djensenius/WorldCup-2026/commit/846de34602f5a4582ee4234e4d42de09bdea02e7))
* update ratatui to avoid vulnerable lru ([#6](https://github.com/djensenius/WorldCup-2026/issues/6)) ([9b26b30](https://github.com/djensenius/WorldCup-2026/commit/9b26b30c0649da028745836c776058626e4bb2a8))

## [0.2.0](https://github.com/djensenius/WorldCup-2026/compare/v0.1.0...v0.2.0) (2026-06-13)


### Features

* scaffold World Cup 2026 TUI workspace ([4593a80](https://github.com/djensenius/WorldCup-2026/commit/4593a80ce8d31def56bec04d0e556ee5145a2c8c))
* team view, favourites, navigation, live activity card, and national flags ([8ac6b2a](https://github.com/djensenius/WorldCup-2026/commit/8ac6b2a77d878f3e2482cfcbe886f018aafe3eb0))
* **wc-data:** implement ESPN, API-Football, and football-data backends ([396789e](https://github.com/djensenius/WorldCup-2026/commit/396789e462f99f6e85f030b2390272fad32885b6))
* **worldcup26:** add offline cache and mouse support ([330f793](https://github.com/djensenius/WorldCup-2026/commit/330f7933fb48607dd45da30d24a14981e3b5a928))
* **worldcup26:** implement matches, live, and match-detail screens ([ff3b908](https://github.com/djensenius/WorldCup-2026/commit/ff3b908b3227b7adc81e295c02400024c2b07dc4))
* **worldcup26:** implement standings and bracket screens ([4bbe216](https://github.com/djensenius/WorldCup-2026/commit/4bbe21693399b1eca72134cff5f81995c560e36b))


### Bug Fixes

* clean up clippy lints and surface stage and data freshness ([6a7f8b2](https://github.com/djensenius/WorldCup-2026/commit/6a7f8b223f3c493d2a09f0c158eb71c91c46a69b))


### Documentation

* add architecture, data-providers, and keybindings guides ([b4b0d80](https://github.com/djensenius/WorldCup-2026/commit/b4b0d8057befce00b881321f3db592cb74211c46))
