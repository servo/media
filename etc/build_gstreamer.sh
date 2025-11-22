#!/bin/sh

# Set error handling
set -e

# Check if the path argument is provided
if [ -z "$1" ]; then
    echo "Usage: $0 <path>"
    exit 1
fi

# Change directory to the provided path
cd "$1" || exit 1

# Clone gstreamer repository
git clone --depth=1 https://gitlab.freedesktop.org/gstreamer/gstreamer.git
cd gstreamer || exit 1

# Build gstreamer
meson setup ../gst-build
ninja -C ../gst-build
ninja -C ../gst-build install