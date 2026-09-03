#!/usr/bin/bash
# SPDX-License-Identifier: AGPL-3.0-or-later

set -o errexit -o nounset -o pipefail

CARGO_AUDIT_VERSION="0.22.2"
CARGO_DENY_VERSION="0.20.2"
RUMDL_VERSION="0.2.64"
SHELLCHECK_VERSION="0.11.0"
SHFMT_VERSION="3.14.0"
YAMLFMT_VERSION="0.21.0"
YAMLLINT_VERSION="1.38.0"

# User-owned bin dir on PATH
BIN_DIR="${HOME}/.local/bin"
mkdir --parents "${BIN_DIR}"
echo "${BIN_DIR}" >>"${GITHUB_PATH}"

echo -e "\n[cargo-audit]"
gh release download "cargo-audit/v${CARGO_AUDIT_VERSION}" --repo rustsec/rustsec \
  --pattern "cargo-audit-x86_64-unknown-linux-gnu-v${CARGO_AUDIT_VERSION}.tgz" \
  --output /tmp/cargo-audit.tgz
tar --extract --file /tmp/cargo-audit.tgz --directory /tmp --strip-components=1 \
  "cargo-audit-x86_64-unknown-linux-gnu-v${CARGO_AUDIT_VERSION}/cargo-audit"
install --mode=755 /tmp/cargo-audit "${BIN_DIR}/cargo-audit"

echo -e "\n[cargo-deny]"
gh release download "${CARGO_DENY_VERSION}" --repo EmbarkStudios/cargo-deny \
  --pattern "cargo-deny-${CARGO_DENY_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
  --output /tmp/cargo-deny.tar.gz
tar --extract --file /tmp/cargo-deny.tar.gz --directory /tmp --strip-components=1 \
  "cargo-deny-${CARGO_DENY_VERSION}-x86_64-unknown-linux-musl/cargo-deny"
install --mode=755 /tmp/cargo-deny "${BIN_DIR}/cargo-deny"

echo -e "\n[rumdl]"
gh release download "v${RUMDL_VERSION}" --repo rvben/rumdl \
  --pattern "rumdl-v${RUMDL_VERSION}-x86_64-unknown-linux-gnu.tar.gz" \
  --output /tmp/rumdl.tar.gz
tar --extract --file /tmp/rumdl.tar.gz --directory /tmp rumdl
install --mode=755 /tmp/rumdl "${BIN_DIR}/rumdl"

echo -e "\n[shellcheck]"
gh release download "v${SHELLCHECK_VERSION}" --repo koalaman/shellcheck \
  --pattern "shellcheck-v${SHELLCHECK_VERSION}.linux.x86_64.tar.gz" \
  --output /tmp/shellcheck.tar.gz
tar --extract --file /tmp/shellcheck.tar.gz --directory /tmp --strip-components=1 \
  "shellcheck-v${SHELLCHECK_VERSION}/shellcheck"
install --mode=755 /tmp/shellcheck "${BIN_DIR}/shellcheck"

echo -e "\n[shfmt]"
gh release download "v${SHFMT_VERSION}" --repo mvdan/sh \
  --pattern "shfmt_v${SHFMT_VERSION}_linux_amd64" \
  --output /tmp/shfmt
install --mode=755 /tmp/shfmt "${BIN_DIR}/shfmt"

echo -e "\n[yamlfmt]"
gh release download "v${YAMLFMT_VERSION}" --repo google/yamlfmt \
  --pattern "yamlfmt_${YAMLFMT_VERSION}_Linux_x86_64.tar.gz" \
  --output /tmp/yamlfmt.tar.gz
tar --extract --file /tmp/yamlfmt.tar.gz --directory /tmp yamlfmt
install --mode=755 /tmp/yamlfmt "${BIN_DIR}/yamlfmt"

echo -e "\n[yamllint]"
PIPX_HOME="${HOME}/.pipx" PIPX_BIN_DIR="${BIN_DIR}" pipx install --force "yamllint==${YAMLLINT_VERSION}"

echo -e "\n[verify]"
for pair in \
  "cargo-audit=${CARGO_AUDIT_VERSION}" \
  "cargo-deny=${CARGO_DENY_VERSION}" \
  "rumdl=${RUMDL_VERSION}" \
  "shellcheck=${SHELLCHECK_VERSION}" \
  "shfmt=${SHFMT_VERSION}" \
  "yamlfmt=${YAMLFMT_VERSION}" \
  "yamllint=${YAMLLINT_VERSION}"; do
  tool="${pair%%=*}"
  want="${pair#*=}"
  if ! got="$("${BIN_DIR}/${tool}" --version 2>&1)"; then
    echo "${tool} failed to report a version"
    exit 1
  fi
  if [[ "${got}" != *"${want}"* ]]; then
    echo "${tool}: want ${want}, got ${got}"
    exit 1
  fi
  echo "${tool} ${want}"
done
