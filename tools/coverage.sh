#!/bin/sh
set -e

minimum=90

for manifest in crates/*/Cargo.toml; do
    crate=$(basename "$(dirname "$manifest")")
    echo "Coverage of $crate"
    cargo llvm-cov --locked --package "$crate" --fail-under-lines "$minimum" \
        --ignore-filename-regex '(^|[/\\])(view|main)\.rs$'
done
