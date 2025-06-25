# ovc - OpenShift Client Version Manager

A command-line tool for managing OpenShift 4 client versions.

## Features

- **Download and manage multiple oc versions** - Download any available OpenShift 4 client version from mirror.openshift.com
- **Cross-platform support** - Works on Linux (x86_64, ARM64) and macOS (x86_64, ARM64) with automatic platform detection
- **Version pattern matching** - List available versions matching major.minor patterns

## Installation

- Grab a release from the [Releases Page](https://github.com/t-c-l-o-u-d/ovc/releases)

## Examples

- Download latest patch version of 4.19
    ```bash
    ovc 4.19
    ```

- Download specific version
    ```bash
    ovc --list 4.14 | fzf | xargs ovc
    ```

## References
- https://mirror.openshift.com/pub/openshift-v4/OpenShift_Release_Types.pdf