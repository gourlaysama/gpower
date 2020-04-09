#!/bin/sh

set -euf

NAME="$1"
shift

HOST_TRIPLET=$(rustc -Vv | awk '/host/ { print $2 }')
DIST_DIR_NAME="$NAME-$HOST_TRIPLET"
DIST_DIR="$MESON_BUILD_ROOT/$DIST_DIR_NAME"
TARGET_FILE="$MESON_BUILD_ROOT/$NAME-$HOST_TRIPLET.tar.gz"

rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

DESTDIR="$DIST_DIR" ninja -C "$MESON_BUILD_ROOT" install

find "$DIST_DIR" -mindepth 2 -type f -exec mv '{}' "$DIST_DIR" ';'
find "$DIST_DIR" -mindepth 1 -depth -type d -exec rm -r '{}' ';'

while [ $# -gt 0 ]
do
    cp -p -t "$DIST_DIR" "$MESON_SOURCE_ROOT/$1"
    shift
done
echo "Packaging into $TARGET_FILE"
tar -cf "$TARGET_FILE" -C "$MESON_BUILD_ROOT" "$DIST_DIR_NAME"