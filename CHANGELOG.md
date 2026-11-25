# Changelog

Following [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Support the `exit` builtin for `$` with exit codes from 0 to 255, including
  captured output and line streaming, without terminating the script.

- Add `process.exit(code?)` to exit immediately with an integer code from 0 to
  255, defaulting to `process.exitCode`, without waiting for pending work.

## [0.2.0-alpha.3] - 2026-11-25

### Added

- Extend S3 with conditional uploads, metadata, configurable request timeouts,
  HEAD statistics, existence checks, deletion, binary helpers, and file presigning.

- Added the immutable `Semver` class with version parsing, bump operations, and string conversion.

### Fixed

- Use Hickory pure-Rust DNS resolution for fetch and S3 instead of system
  getaddrinfo, avoiding musl resolver limitations for internal hostnames.
  Use system DNS configuration without falling back to public DNS servers.

## [0.2.0-alpha.2] - 2026-11-25

### Changed

- Replace the Boa global `process` with `import { process } from "snek"`.
  Add process metadata, shared working-directory changes, runtime-local mutable
  environment variables inherited by child commands, and graceful `exitCode`.
  The existing `argv` export shares the same array as `process.argv`.

### Added

- Add lazy `file()` references with asynchronous text, JSON, binary, and Blob
  reads, overwrite and append writes, and existence checks.

- Support CAC-style CLI commands, aliases, positional arguments, command options,
  and synchronous or asynchronous actions.

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
