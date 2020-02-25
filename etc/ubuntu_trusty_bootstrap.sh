#!/usr/bin/env bash

# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.

set -o errexit

if [ ! -d "gst" ]; then
  curl -L https://servo-deps.s3.amazonaws.com/gstreamer/gstreamer-1.14-x86_64-linux-gnu.20190213.tar.gz | tar xz
  sed -i "s;prefix=/opt/gst;prefix=$PWD/gst;g" $PWD/gst/lib/pkgconfig/*.pc
fi
export PKG_CONFIG_PATH=$PWD/gst/lib/pkgconfig
export GST_PLUGIN_SYSTEM_PATH=$PWD/gst/lib/gstreamer-1.0
export GST_PLUGIN_SCANNER=$PWD/gst/libexec/gstreamer-1.0/gst-plugin-scanner
export PATH=$PATH:$PWD/gst/bin
export LD_LIBRARY_PATH=$PWD/gst/lib:$LD_LIBRARY_PATH

