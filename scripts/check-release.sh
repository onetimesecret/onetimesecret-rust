#!/usr/bin/env bash
# Verify that the version in Cargo.toml, the newest CHANGELOG.md entry and
# (when given) a vX.Y.Z tag all agree. CI runs this before publishing;
# run it locally before tagging.
#
#   scripts/check-release.sh            # Cargo.toml vs CHANGELOG.md
#   scripts/check-release.sh v0.2.0     # …and vs the tag
set -euo pipefail

cd "$(dirname "$0")/.."

cargo_version=$(cargo metadata --no-deps --format-version 1 \
  | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')

changelog_version=$(grep -m1 -E '^## \[[0-9]+\.[0-9]+\.[0-9]+' CHANGELOG.md \
  | sed -E 's/^## \[([^]]+)\].*/\1/' || true)

status=0
echo "Cargo.toml:   $cargo_version"
echo "CHANGELOG.md: ${changelog_version:-<no released entry>}"

if [ "$cargo_version" != "$changelog_version" ]; then
  echo "error: CHANGELOG.md's newest release entry does not match Cargo.toml" >&2
  status=1
fi

if [ "${1:-}" != "" ]; then
  tag="${1#v}"
  echo "tag:          v$tag"
  if [ "$tag" != "$cargo_version" ]; then
    echo "error: tag v$tag does not match Cargo.toml version $cargo_version" >&2
    status=1
  fi
  if ! grep -qE "^## \[$cargo_version\] - [0-9]{4}-[0-9]{2}-[0-9]{2}" CHANGELOG.md; then
    echo "error: CHANGELOG.md entry for $cargo_version needs a YYYY-MM-DD release date" >&2
    status=1
  fi
fi

exit $status
