#!/usr/bin/bash
# SPDX-License-Identifier: AGPL-3.0-or-later

set -o errexit -o nounset -o pipefail

CARGO_DENY_VERSION="0.20.2"
CARGO_DENY_SHA256="9f12ed4c49936e09b48bf862b595cde2fe64fcbd9d74dfacac6131ca824c8d5f"
RUMDL_VERSION="0.2.64"
RUMDL_SHA256="1eda26691fa11c020a274f660fb35a0cf7556060edbef49d77504c4b838cce9a"
RUST_VERSION="$(grep --max-count=1 '^channel' rust-toolchain.toml | cut --delimiter='"' --fields=2)"
SHELLCHECK_VERSION="0.11.0"
SHELLCHECK_SHA256="b7af85e41cc99489dcc21d66c6d5f3685138f06d34651e6d34b42ec6d54fe6f6"
SHFMT_VERSION="3.14.0"
SHFMT_SHA256="fe42021c7272ef2d67ea36cbc3031683c625d0badec733ef3a57b567246a0b66"
YAMLFMT_VERSION="0.21.0"
YAMLFMT_SHA256="1f300d9257b232bb3b541d7fb1b0e6b3c121bcbab381c86cd38cb8722be8a566"
YAMLLINT_VERSION="1.38.0"

# User-owned bin dir on PATH
BIN_DIR="${HOME}/.local/bin"
mkdir --parents "${BIN_DIR}"
echo "${BIN_DIR}" >>"${GITHUB_PATH}"

# Reject downloads with unexpected hash
verify_sha256() {
  local file="${1}" want="${2}" got
  got="$(sha256sum "${file}" | cut --delimiter=' ' --fields=1)"
  if [[ "${got}" != "${want}" ]]; then
    echo "${file##*/}: want ${want}, got ${got}"
    exit 1
  fi
}

echo -e "\n[cargo-deny]"
gh release download "${CARGO_DENY_VERSION}" --repo EmbarkStudios/cargo-deny \
  --pattern "cargo-deny-${CARGO_DENY_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
  --output /tmp/cargo-deny.tar.gz
verify_sha256 /tmp/cargo-deny.tar.gz "${CARGO_DENY_SHA256}"
tar --extract --file /tmp/cargo-deny.tar.gz --directory /tmp --strip-components=1 \
  "cargo-deny-${CARGO_DENY_VERSION}-x86_64-unknown-linux-musl/cargo-deny"
install --mode=755 /tmp/cargo-deny "${BIN_DIR}/cargo-deny"

echo -e "\n[rumdl]"
gh release download "v${RUMDL_VERSION}" --repo rvben/rumdl \
  --pattern "rumdl-v${RUMDL_VERSION}-x86_64-unknown-linux-gnu.tar.gz" \
  --output /tmp/rumdl.tar.gz
verify_sha256 /tmp/rumdl.tar.gz "${RUMDL_SHA256}"
tar --extract --file /tmp/rumdl.tar.gz --directory /tmp rumdl
install --mode=755 /tmp/rumdl "${BIN_DIR}/rumdl"

echo -e "\n[rust]"
MSRV="$(grep --max-count=1 '^rust-version' Cargo.toml | cut --delimiter='"' --fields=2)"
if [[ "${MSRV}" != "${RUST_VERSION}" ]]; then
  echo "Toolchain ${RUST_VERSION} does not match MSRV ${MSRV}"
  exit 1
fi
rustup --quiet toolchain install --no-self-update

echo -e "\n[shellcheck]"
gh release download "v${SHELLCHECK_VERSION}" --repo koalaman/shellcheck \
  --pattern "shellcheck-v${SHELLCHECK_VERSION}.linux.x86_64.tar.gz" \
  --output /tmp/shellcheck.tar.gz
verify_sha256 /tmp/shellcheck.tar.gz "${SHELLCHECK_SHA256}"
tar --extract --file /tmp/shellcheck.tar.gz --directory /tmp --strip-components=1 \
  "shellcheck-v${SHELLCHECK_VERSION}/shellcheck"
install --mode=755 /tmp/shellcheck "${BIN_DIR}/shellcheck"

echo -e "\n[shfmt]"
gh release download "v${SHFMT_VERSION}" --repo mvdan/sh \
  --pattern "shfmt_v${SHFMT_VERSION}_linux_amd64" \
  --output /tmp/shfmt
verify_sha256 /tmp/shfmt "${SHFMT_SHA256}"
install --mode=755 /tmp/shfmt "${BIN_DIR}/shfmt"

echo -e "\n[yamlfmt]"
gh release download "v${YAMLFMT_VERSION}" --repo google/yamlfmt \
  --pattern "yamlfmt_${YAMLFMT_VERSION}_Linux_x86_64.tar.gz" \
  --output /tmp/yamlfmt.tar.gz
verify_sha256 /tmp/yamlfmt.tar.gz "${YAMLFMT_SHA256}"
tar --extract --file /tmp/yamlfmt.tar.gz --directory /tmp yamlfmt
install --mode=755 /tmp/yamlfmt "${BIN_DIR}/yamlfmt"

echo -e "\n[yamllint]"
PIPX_HOME="${HOME}/.pipx" PIPX_BIN_DIR="${BIN_DIR}" pipx install --force --quiet "yamllint==${YAMLLINT_VERSION}"

verify_version() {
  local bin="${1}" want="${2}" got
  if ! got="$("${bin}" --version 2>&1)"; then
    echo "${bin##*/} failed to report a version"
    exit 1
  fi
  if [[ "${got}" != *"${want}"* ]]; then
    echo "${bin##*/}: want ${want}, got ${got}"
    exit 1
  fi
  echo "${bin##*/} ${want}"
}

echo -e "\n[verify]"
verify_version "$(command -v rustc)" "${RUST_VERSION}"
for pair in \
  "cargo-deny=${CARGO_DENY_VERSION}" \
  "rumdl=${RUMDL_VERSION}" \
  "shellcheck=${SHELLCHECK_VERSION}" \
  "shfmt=${SHFMT_VERSION}" \
  "yamlfmt=${YAMLFMT_VERSION}" \
  "yamllint=${YAMLLINT_VERSION}"; do
  verify_version "${BIN_DIR}/${pair%%=*}" "${pair#*=}"
done
