# Changelog

Following [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Add lazy `file()` references with asynchronous text, JSON, binary, and Blob
  reads, overwrite and append writes, and existence checks.

- Support `import.meta.dir`, `file`, `path`, and `url` with TypeScript
  declarations.

## [0.2.0-alpha.1] - 2026-11-25

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

[Unreleased]: https://github.com/PeterlitsZo/Snek/compare/v0.2.0-alpha.1...HEAD
[0.2.0-alpha.1]: https://github.com/PeterlitsZo/Snek/compare/v0.1.0...v0.2.0-alpha.1
[0.1.0]: https://github.com/PeterlitsZo/Snek/releases/tag/v0.1.0
