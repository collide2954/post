#!/bin/sh
set -e

crates="post-proto post-crypto post-rules"

if [ "$1" = "--staged" ]; then
    shift
    pattern='^(Cargo\.toml|Cargo\.lock|rust-toolchain\.toml|tools/proofs\.sh|\.githooks/'
    for crate in $crates; do
        pattern="$pattern|crates/$crate/"
    done
    if ! git diff --cached --name-only | grep -q -E "$pattern)"; then
        echo "The commit changes no proved crate. The proofs did not run."
        exit 0
    fi
fi

if ! command -v cargo-verus >/dev/null 2>&1; then
    for directory in "$POST_VERUS" "$HOME"/tools/verus/verus-*; do
        if [ -x "$directory/cargo-verus" ] || [ -x "$directory/cargo-verus.exe" ]; then
            PATH="$directory:$PATH"
            export PATH
            break
        fi
    done
fi

if ! command -v cargo-verus >/dev/null 2>&1; then
    echo "Verus is not on this computer. The proofs did not run."
    if [ "$1" = "--required" ]; then
        exit 1
    fi
    exit 0
fi

for crate in $crates; do
    cargo verus verify --package "$crate"
done
