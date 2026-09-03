#!/usr/bin/bash
# SPDX-License-Identifier: AGPL-3.0-or-later

set -o errexit -o nounset -o pipefail

VERSION="${1:-}"
ARCH="${2:-linux-x86_64}"

if [[ -z "${VERSION}" ]]; then
  echo "::error::Provide a version to release"
  exit 1
fi

echo -e "\n[release]"
gh release create "v${VERSION}" \
  --title "v${VERSION}" \
  --generate-notes \
  "ovc-${ARCH}" \
  "ovc-${ARCH}.sha256"
