#!/usr/bin/bash
# SPDX-License-Identifier: AGPL-3.0-or-later

set -o errexit -o nounset -o pipefail

CHECK_ONLY="false"
if [[ "${1:-}" == "--check" ]]; then
  CHECK_ONLY="true"
fi

mapfile -t WORKFLOW_FILES < <(git ls-files '.github/workflows/*.yaml')

# Resolve latest release commit and tag
latest_pin() {
  local repo="$1" tag
  tag="$(gh api "repos/${repo}/releases/latest" --jq '.tag_name')"
  printf '%s %s\n' \
    "$(gh api "repos/${repo}/commits/${tag}" --jq '.sha')" "${tag}"
}

mapfile -t PINS < <(
  grep --no-filename --only-matching --extended-regexp \
    'uses: [^@]+@[0-9a-f]{40}' "${WORKFLOW_FILES[@]}" \
    | sed 's/uses: //' | sort --unique
)

OUTDATED="false"

for pin in "${PINS[@]}"; do
  repo="${pin%@*}"
  old_sha="${pin#*@}"
  read -r new_sha tag < <(latest_pin "${repo}")

  if [[ "${old_sha}" == "${new_sha}" ]]; then
    printf 'ok        %s %s\n' "${repo}" "${tag}"
    continue
  fi

  OUTDATED="true"
  printf 'outdated  %s %s\n' "${repo}" "${tag}"

  if [[ "${CHECK_ONLY}" == "true" ]]; then
    continue
  fi

  # Rewrite pin and trailing version comment
  sed --in-place --regexp-extended \
    "s|@${old_sha}([[:space:]]*#.*)?$|@${new_sha} # ${tag}|" \
    "${WORKFLOW_FILES[@]}"
done

if [[ "${CHECK_ONLY}" == "true" && "${OUTDATED}" == "true" ]]; then
  exit 1
fi
