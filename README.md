# ovc - OpenShift Client Version Control

A command-line tool for managing OpenShift client versions.

## Installation

- Grab a release from the
  [Releases Page](https://github.com/t-c-l-o-u-d/ovc/releases)

- Make it executable and put it somewhere on your `$PATH`
  (e.g. `~/.local/bin`)

  ```bash
  curl -fLsS --create-dirs -o ~/.local/bin/ovc \
    https://github.com/t-c-l-o-u-d/ovc/releases/latest/download/ovc-linux-x86_64 \
    && chmod +x ~/.local/bin/ovc
  ```

## Usage

- Download latest patch version of 5.0

  ```bash
  ovc 5.0
  ```

- Download a specific version

  ```bash
  ovc --list 4.22 | fzf | xargs ovc
  ```

- Download matching `oc` directly from a currently
  connected cluster

  ```bash
  ovc --match-server
  ```

- View the manual

  ```bash
  man ovc
  ```

  The man page is installed automatically on first run.

- Enable bash completion

  ```bash
  mkdir --parents ~/.local/share/bash-completion/completions &&\
  ovc --completion bash > ~/.local/share/bash-completion/completions/ovc
  ```

- Remove all installed versions (except the active one)

  ```bash
  ovc --prune
  ```

## References

- [OpenShift Release Types](https://mirror.openshift.com/pub/openshift-v4/OpenShift_Release_Types.pdf)
