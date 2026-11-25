#!/usr/bin/env bash

set -euo pipefail

release_base="https://peterlits.net/snek/release"
install_dir="${HOME}/.local/bin"
release_tag=""
tmpdir=""
staged_binary=""

fail() {
  printf 'Error: %s\n' "$*" >&2
  exit 1
}

cleanup() {
  if [ -n "$tmpdir" ]; then
    rm -rf -- "$tmpdir"
  fi
  if [ -n "$staged_binary" ]; then
    rm -f -- "$staged_binary"
  fi
}

validate_tag() {
  case "$1" in
    ''|v|*[!a-zA-Z0-9.+-]*) fail "Invalid release tag: $1" ;;
    v[0-9]*) ;;
    *) fail "Release tags must start with v followed by a number" ;;
  esac
}

download() {
  local -a display=(--silent --show-error)
  if [ "${3:-}" = progress ] && [ -t 2 ]; then
    display=(--progress-bar --show-error)
  fi

  curl --fail --location --retry 3 "${display[@]}" \
    --proto '=https' --proto-redir '=https' "$1" -o "$2" ||
    fail "Failed to download $1"
}

usage() {
  cat <<'USAGE'
Usage: install.sh [--bin-dir=<path>] [<tag>]

Install the latest stable release, or specify a tag such as v0.1.0.

Options:
  --bin-dir=<path>   Install into <path> instead of ~/.local/bin.
  -h, --help        Show this help message.
USAGE
}

parse_args() {
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --bin-dir)
        [ "$#" -ge 2 ] || fail '--bin-dir requires a path'
        install_dir="$2"
        shift 2
        ;;
      --bin-dir=*)
        install_dir="${1#--bin-dir=}"
        shift
        ;;
      --help|-h)
        usage
        exit 0
        ;;
      v*)
        [ -z "$release_tag" ] || fail 'Specify only one release tag'
        validate_tag "$1"
        release_tag="$1"
        shift
        ;;
      *) fail "Unknown argument: $1" ;;
    esac
  done
  [ -n "$install_dir" ] || fail 'Install directory must not be empty'
  case "$install_dir" in
    /*) ;;
    *) install_dir="$PWD/$install_dir" ;;
  esac
}

check_dependencies() {
  local command

  for command in curl tar; do
    command -v "$command" >/dev/null 2>&1 || fail "Required command not found: $command"
  done
  if ! command -v sha256sum >/dev/null 2>&1 &&
     ! command -v shasum >/dev/null 2>&1; then
    fail 'sha256sum or shasum is required'
  fi
}

detect_asset() {
  local os arch

  case "$(uname -s)" in
    Linux) os=linux ;;
    Darwin) os=macos ;;
    *) fail 'Only Linux and macOS are supported' ;;
  esac
  case "$(uname -m)" in
    x86_64|amd64) arch=amd64 ;;
    aarch64|arm64) arch=arm64 ;;
    *) fail 'Only x86_64 and ARM64 are supported' ;;
  esac

  if [ "$os" = macos ] && [ "$arch" = amd64 ]; then
    if [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || true)" = 1 ]; then
      arch=arm64
      printf 'Rosetta detected; installing the native ARM64 binary.\n' >&2
    fi
  fi

  printf '%s-%s\n' "$os" "$arch"
}

verify_archive() {
  local archive_path checksums_path archive expected actual hash name

  local -a checksum
  archive_path="$1"
  checksums_path="$2"
  archive="${archive_path##*/}"

  if command -v sha256sum >/dev/null 2>&1; then
    checksum=(sha256sum)
  elif command -v shasum >/dev/null 2>&1; then
    checksum=(shasum -a 256)
  else
    fail 'sha256sum or shasum is required'
  fi

  expected=""
  while read -r hash name; do
    if [ "$name" = "./$archive" ] || [ "$name" = "$archive" ]; then
      [ -z "$expected" ] || fail 'Duplicate checksum entry'
      expected="$hash"
    fi
  done < "$checksums_path"
  [ -n "$expected" ] || fail 'Archive checksum is missing'
  actual="$("${checksum[@]}" "$archive_path")"
  [ "${actual%% *}" = "$expected" ] || fail 'Archive checksum does not match'
}

install_binary() {
  local archive_path="$1"

  tar -xzf "$archive_path" -C "$tmpdir" snek ||
    fail "Failed to extract $archive_path"
  [ -f "$tmpdir/snek" ] && [ ! -L "$tmpdir/snek" ] || fail 'Archive must contain a regular snek binary'
  mkdir -p "$install_dir" || fail "Failed to create $install_dir"
  staged_binary="$(mktemp "$install_dir/.snek.XXXXXX")" ||
    fail "Failed to create a temporary file in $install_dir"
  cp "$tmpdir/snek" "$staged_binary" || fail "Failed to copy the binary to $staged_binary"
  chmod 0755 "$staged_binary" || fail "Failed to make $staged_binary executable"
  [ ! -d "$install_dir/snek" ] || fail 'Destination is a directory'
  mv -f "$staged_binary" "$install_dir/snek" || fail "Failed to install $install_dir/snek"
  staged_binary=""
}

show_next_steps() {
  local quoted_dir shell_name

  case ":${PATH:-}:" in
    *":$install_dir:"*) ;;
    *)
      shell_name="${SHELL:-sh}"
      shell_name="${shell_name##*/}"
      printf '\nAdd Snek to PATH for this session:\n'
      case "$shell_name" in
        fish)
          quoted_dir="${install_dir//\\/\\\\}"
          quoted_dir="${quoted_dir//\'/\\\'}"
          printf "  set -gx PATH '%s' \$PATH\n" "$quoted_dir"
          printf 'To make this permanent, add that line to your fish configuration.\n'
          ;;
        *)
          printf '  export PATH=%q:"$PATH"\n' "$install_dir"
          printf 'To make this permanent, add that line to your shell startup file.\n'
          ;;
      esac
      ;;
  esac
  printf '\nRun snek --help to get started.\n'
}

main() {
  local asset archive release_url

  parse_args "$@"
  check_dependencies
  asset="$(detect_asset)"

  tmpdir="$(mktemp -d)"
  trap cleanup EXIT
  trap 'exit 130' INT
  trap 'exit 143' TERM
  if [ -z "$release_tag" ]; then
    printf 'Checking the latest Snek release...\n'
    download "$release_base/LATEST" "$tmpdir/LATEST"
    release_tag="$(cat "$tmpdir/LATEST")"
    validate_tag "$release_tag"
  fi
  archive="snek-${asset}.tar.gz"
  release_url="$release_base/$release_tag"
  printf 'Downloading Snek %s for %s\n' "$release_tag" "$asset"
  download "$release_url/$archive" "$tmpdir/$archive" progress
  download "$release_url/SHA256SUMS" "$tmpdir/SHA256SUMS"

  printf 'Verifying archive checksum...\n'
  verify_archive "$tmpdir/$archive" "$tmpdir/SHA256SUMS"
  install_binary "$tmpdir/$archive"

  printf 'Installed Snek %s to %s/snek\n' "$release_tag" "$install_dir"
  show_next_steps
}

main "$@"
