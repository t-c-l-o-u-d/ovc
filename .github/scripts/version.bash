#!/usr/bin/bash
# SPDX-License-Identifier: AGPL-3.0-or-later

set -o errexit -o nounset -o pipefail

VERSION="${1:-}"
SEMVER='^[0-9]+\.[0-9]+\.[0-9]+$'

echo -e "\n[validate]"
if [[ ! "${VERSION}" =~ ${SEMVER} ]]; then
  echo "::error::Invalid semver (e.g. 1.2.0)"
  exit 1
fi
echo "${VERSION}"

echo -e "\n[version]"
sed --in-place "s/^version = .*/version = \"${VERSION}\"/" Cargo.toml

echo -e "\n[lockfile]"
cargo generate-lockfile

echo -e "\n[commit]"
if git diff --quiet Cargo.toml; then
  echo "No version change to commit"
else
  git config user.name "github-actions[bot]"
  git config user.email "github-actions[bot]@users.noreply.github.com"
  git add Cargo.toml Cargo.lock
  git commit --message "Bump version to ${VERSION} [skip ci]"
  git push
fi
