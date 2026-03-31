# Changelog

## [0.7.0](https://github.com/JonasKs/portmap/compare/portmap-v0.6.1...portmap-v0.7.0) (2026-03-31)


### Features

* add ~/.config/portmap/config.toml ([#11](https://github.com/JonasKs/portmap/issues/11)) ([6c14c66](https://github.com/JonasKs/portmap/commit/6c14c660a4e6d34d721b4bb969a921da50d1de42))
* Docker/Podman container port discovery ([#12](https://github.com/JonasKs/portmap/issues/12)) ([1a66f69](https://github.com/JonasKs/portmap/commit/1a66f69dc4a0d32364a7be4b9c01d6159eae2c68))

## [0.6.1](https://github.com/JonasKs/portmap/compare/portmap-v0.6.0...portmap-v0.6.1) (2026-03-31)


### Bug Fixes

* trigger release for scanning and UI improvements ([3c180e4](https://github.com/JonasKs/portmap/commit/3c180e42ced4a79235671a6f4bd4b641dee500fb))

## [0.6.0](https://github.com/JonasKs/portmap/compare/portmap-v0.5.1...portmap-v0.6.0) (2026-03-31)


### Features

* adaptive scanning, scan status SSE, and UI polish ([5143907](https://github.com/JonasKs/portmap/commit/51439078009ffd47e984d28400ddc784defc1e79))


### Bug Fixes

* reduce scan batch size to prevent false negatives ([173ad47](https://github.com/JonasKs/portmap/commit/173ad4751b838dc78547687edb37bc8d58347216))

## [0.5.1](https://github.com/JonasKs/portmap/compare/portmap-v0.5.0...portmap-v0.5.1) (2026-03-30)


### Bug Fixes

* port as first column in portmap list ([576426d](https://github.com/JonasKs/portmap/commit/576426d9673c63b260d8ab9e40d12e7ca7fa8fdf))
* restore aligned columns and header in portmap list ([c417c48](https://github.com/JonasKs/portmap/commit/c417c48a15557a2896558631fdf499543eff54bf))

## [0.5.0](https://github.com/JonasKs/portmap/compare/portmap-v0.4.0...portmap-v0.5.0) (2026-03-30)


### Features

* add centered GitHub logo to dashboard footer ([f2dfe92](https://github.com/JonasKs/portmap/commit/f2dfe92855d2052a0932c7d09f5450eb25321aa5))
* add kill command, merge scan into list, resolve by port or name ([715f3f4](https://github.com/JonasKs/portmap/commit/715f3f4014e294c1693d29dd5cc7dc7c24a1a01c))
* kill processes from dashboard UI ([6f9c73b](https://github.com/JonasKs/portmap/commit/6f9c73b46cf5a7db1d4d56eeec5bc14ad955a570))
* row action hierarchy with edit + overflow menu ([aa7894e](https://github.com/JonasKs/portmap/commit/aa7894e154db75c15dbf93ac759c5f46abb28eae))
* table output for list, smarter status, unique names ([5510f12](https://github.com/JonasKs/portmap/commit/5510f123d5d5de582b639872ca90734eac4e63ea))
* unified color dots, row highlights, and clearer row states ([1b08b93](https://github.com/JonasKs/portmap/commit/1b08b93a40d75cac825159e5875365424d668a70))


### Bug Fixes

* check if portmap is actually running in list output ([ab11cae](https://github.com/JonasKs/portmap/commit/ab11cae75b0d4ee512d040af7f4eb592ec6b70b6))
* enforce unique app names via partial index ([23fdbff](https://github.com/JonasKs/portmap/commit/23fdbffd45102c069a3d8405f8c7b5a1586a7f0c))
* only kill listening process, not clients connected to the port ([d88dcf7](https://github.com/JonasKs/portmap/commit/d88dcf7faaa6cc0ecee411741f95c6ba39933394))
* only show dashboard URL in status when running ([3841121](https://github.com/JonasKs/portmap/commit/3841121635b82d47290fcaa33c526764699472ad))
* plain output for list (LLM-friendly), table for status (human) ([4bc6e84](https://github.com/JonasKs/portmap/commit/4bc6e848e5dd127e2438d5aa700af243101b60fa))
* reorder context menu — kill at bottom in red ([0efd945](https://github.com/JonasKs/portmap/commit/0efd945fae93fc6474109caab1867692695c7ff0))
* right-click edit by stopping event propagation ([a9138f8](https://github.com/JonasKs/portmap/commit/a9138f857df6cca646fcbe418f9735f141324383))
* scan both IPv4 and IPv6 loopback for port detection ([cb2a877](https://github.com/JonasKs/portmap/commit/cb2a8777c912eeba8c3a049f0ab39b1f7efae293))

## [0.4.0](https://github.com/JonasKs/portmap/compare/portmap-v0.3.2...portmap-v0.4.0) (2026-03-30)


### Features

* show online status and sort by port in CLI list ([2c3e892](https://github.com/JonasKs/portmap/commit/2c3e892ff3982b9e6ae6af08f574721f2e93904e))
* SSE live updates with shared scanner and row-level diffing ([4ddadad](https://github.com/JonasKs/portmap/commit/4ddadadd656683d821c7f4abdffbaa9d583bb9cd))

## [0.3.2](https://github.com/JonasKs/portmap/compare/portmap-v0.3.1...portmap-v0.3.2) (2026-03-29)


### Bug Fixes

* detect brew install and redirect to brew services ([636b185](https://github.com/JonasKs/portmap/commit/636b18584543d976638b605be8ff8c73ed1b3ecc))

## [0.3.1](https://github.com/JonasKs/portmap/compare/portmap-v0.3.0...portmap-v0.3.1) (2026-03-29)


### Bug Fixes

* uninstall no longer deletes binary, add brew pre-uninstall hook ([6021b26](https://github.com/JonasKs/portmap/commit/6021b26affef1a0f76205112f9aa4fd9118c1bf8))

## [0.3.0](https://github.com/JonasKs/portmap/compare/portmap-v0.2.1...portmap-v0.3.0) (2026-03-29)


### Features

* optional app names, custom tag colors, and XSS hardening ([bfa8304](https://github.com/JonasKs/portmap/commit/bfa8304a21ff5cc7845c27abcf9c9bd8b64656fa))

## [0.2.1](https://github.com/JonasKs/portmap/compare/portmap-v0.2.0...portmap-v0.2.1) (2026-03-29)


### Bug Fixes

* homebrew formula update workflow ([df6b5a7](https://github.com/JonasKs/portmap/commit/df6b5a7411703e3d855afb5781ba56be353dc70e))

## [0.2.0](https://github.com/JonasKs/portmap/compare/portmap-v0.1.0...portmap-v0.2.0) (2026-03-29)


### Features

* add CLI subcommands for managing apps and services ([1bba31a](https://github.com/JonasKs/portmap/commit/1bba31a189b6858ba72a4c844dfa18a5ab8fee60))
* auto-detect macOS system ports and polish UI ([2d624b0](https://github.com/JonasKs/portmap/commit/2d624b0d6028a28ff624fa340e870776a3a50f77))


### Bug Fixes

* format code and remove Formula directory ([1555895](https://github.com/JonasKs/portmap/commit/1555895c38d5df3c7d7b4563a7f477549a795fb3))
* hover icon visibility and inline edit in Firefox ([08b75af](https://github.com/JonasKs/portmap/commit/08b75aff5368220ed88848dccdf58937778466e6))
