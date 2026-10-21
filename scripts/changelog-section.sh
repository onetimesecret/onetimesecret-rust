#!/usr/bin/env bash
# Print the CHANGELOG.md section for one version, for release notes.
#
#   scripts/changelog-section.sh 0.2.0
set -euo pipefail

version="${1:?usage: changelog-section.sh <version>}"
version="${version#v}"
cd "$(dirname "$0")/.."

section=$(awk -v v="$version" '
  /^## \[/ { if (found) exit; if (index($0, "## [" v "]") == 1) { found = 1; next } }
  found { print }
' CHANGELOG.md)

if [ -z "$section" ]; then
  echo "error: no CHANGELOG.md section for $version" >&2
  exit 1
fi
printf '%s\n' "$section"
