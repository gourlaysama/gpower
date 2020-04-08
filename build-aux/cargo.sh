#!/bin/sh

set -euf -o pipefail

CARGO_PATH="$1"
MESON_BUILD_ROOT="$2"
MESON_SOURCE_ROOT="$3"
APP_OUTPUT="$4"
OPTION_DEBUG="$4"
export RUSTFLAGS=${RUSTFLAGS:-$5}

export CARGO_TARGET_DIR="$MESON_BUILD_ROOT"/cargo-target
export CARGO_HOME="$CARGO_TARGET_DIR"/cargo-home

if test "$OPTION_DEBUG" == "true"
then
    CARGO_ARGS=""
    CARGO_T_DIR="debug"
else
    CARGO_ARGS="--release "
    CARGO_T_DIR="release"
fi

CARGO_ARGS="$CARGO_ARGS --message-format=short"

"$CARGO_PATH" build --manifest-path "$MESON_SOURCE_ROOT"/Cargo.toml $CARGO_ARGS

cp "$CARGO_TARGET_DIR"/$CARGO_T_DIR/gpower-tweaks $APP_OUTPUT