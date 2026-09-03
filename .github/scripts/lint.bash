#!/usr/bin/bash
# SPDX-License-Identifier: AGPL-3.0-or-later

set -o errexit -o nounset -o pipefail

echo -e "\n[cargo-audit]"
cargo audit --deny warnings --deny unmaintained --deny unsound --deny yanked --quiet

echo -e "\n[cargo-check-debug]"
cargo check --all-features --quiet

echo -e "\n[cargo-check-release]"
cargo check --all-features --release --quiet

echo -e "\n[cargo-clippy]"
cargo clippy --all-features --quiet -- \
  --deny warnings \
  --deny clippy::all \
  --deny clippy::correctness \
  --deny clippy::suspicious \
  --deny clippy::complexity \
  --deny clippy::perf \
  --deny clippy::style \
  --deny clippy::pedantic \
  --deny clippy::cargo \
  --allow clippy::doc-markdown \
  --allow clippy::multiple_crate_versions

echo -e "\n[cargo-deny]"
cargo deny check advisories bans sources

echo -e "\n[cargo-fmt]"
cargo fmt -- --check --files-with-diff

echo -e "\n[cargo-test]"
cargo test --release --quiet

echo -e "\n[rumdl]"
rumdl check .

# build list of shell files to lint
mapfile -t SHELL_FILES < <(git ls-files '*.bash' '*.sh')

echo -e "\n[shellcheck]"
shellcheck --external-sources --format=gcc "${SHELL_FILES[@]}"

echo -e "\n[shfmt]"
shfmt --diff --language-dialect=bash --indent 2 --binary-next-line "${SHELL_FILES[@]}"

echo -e "\n[yamlfmt]"
yamlfmt --lint .

echo -e "\n[yamllint]"
yamllint --strict --format auto .
