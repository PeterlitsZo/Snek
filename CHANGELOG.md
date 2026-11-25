# Changelog

Following [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Support `.nothrow()` returning `stdout`, `stderr`, and `exitCode` for shell
  commands.
- Support global `process.cwd()` and `process.env` through Boa.
- Add npm package `@peterlits/snek`.
- Support `cd PATH` in `$` commands.
- Support command-specific environment overrides with `.env()` on `$` commands.

### Changed

- Improve installer progress output.

## [0.1.0] - 2026-11-24

### Added

- Initial release of Snek.

[Unreleased]: https://github.com/PeterlitsZo/Snek/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/PeterlitsZo/Snek/releases/tag/v0.1.0
