#!/usr/bin/env bash

set -euo pipefail

fail() {
  printf 'Error: %s\n' "$*" >&2
  exit 1
}

main() {
  if [ "$#" -ne 1 ]; then
    fail 'Usage: scripts/release.sh VERSION (for example, 0.1.0)'
  fi

  local version="$1"
  local tag="v$version"
  local script_dir branch remote_tags

  case "$version" in
    ''|*[!0-9a-zA-Z.+-]*) fail 'Invalid version' ;;
    [0-9]*) ;;
    *) fail 'VERSION must start with a number and must not include the v prefix' ;;
  esac

  script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
  cd "$script_dir/.."
  git rev-parse --show-toplevel >/dev/null
  branch="$(git symbolic-ref --quiet --short HEAD)" || fail 'Check out a branch before releasing'
  git check-ref-format "refs/tags/$tag" || fail "Invalid tag: $tag"

  python3 - "$version" <<'PY'
import sys
import json
import subprocess

metadata = json.loads(subprocess.check_output([
    'cargo', 'metadata', '--no-deps', '--format-version', '1', '--locked', '--offline',
]))
package = next(package for package in metadata['packages'] if package['name'] == 'snek')
version = package['version']
if sys.argv[1] != version:
    raise SystemExit(f'Error: update Cargo.toml and Cargo.lock to {sys.argv[1]} before releasing (current version: {version})')
PY

  if git show-ref --verify --quiet "refs/tags/$tag"; then
    fail "Tag already exists locally: $tag"
  fi
  remote_tags="$(git ls-remote --tags origin "refs/tags/$tag")" ||
    fail 'Could not check tags on origin'
  if [ -n "$remote_tags" ]; then
    fail "Tag already exists on origin: $tag"
  fi

  git add -A
  git commit --allow-empty -m "chore: Release \"$tag\"."
  git tag "$tag"
  git push --atomic origin "HEAD:refs/heads/$branch" "refs/tags/$tag:refs/tags/$tag"

  printf 'Released %s on origin/%s\n' "$tag" "$branch"
}

main "$@"
