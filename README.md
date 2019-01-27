# Servo Media

[![Build Status](https://travis-ci.org/servo/media.svg?branch=master)](https://travis-ci.org/servo/media)

## Requirements
So far the only supported and default backend is
[GStreamer](https://gstreamer.freedesktop.org/).
So in order to build  this crate you need to install all
[gstreamer-rs](https://github.com/sdroege/gstreamer-rs) dependencies for your
specific platform as listed
[here](https://github.com/sdroege/gstreamer-rs#installation).

### Android
For Android there are some extra requirements.

First of all, you need to install the appropriate toolchain for your target.
The recommended approach is to install it through
[rustup](https://rustup.rs/). Taking `arm-linux-androideabi` as our example
target you need to do:

```bash
rustup target add arm-linux-androideabi
```

In addition to that, you also need to install the Android
[NDK](https://developer.android.com/ndk/guides/).
The recommended NDK version is
[r16b](https://developer.android.com/ndk/downloads/older_releases). The
Android [SDK](https://developer.android.com/studio/) is not mandatory
but recommended for practical development.

Once you have the Android NDK installed in your machine, you need to create
what the NDK itself calls a
[standalone toolchain](https://developer.android.com/ndk/guides/standalone_toolchain).

```bash
 $ ${ANDROID_NDK}/build/tools/make-standalone-toolchain.sh \
   --platform=android-18 --toolchain=arm-linux-androideabi-4.9 \
   --install-dir=android-18-arm-toolchain --arch=arm
```

After that you need to tell Cargo where to find the Android linker and ar,
which is in the standalone NDK toolchain we just created. To do that we
configure the `arm-linux-androideabi` target in `.cargo/config` (or in
`~/.cargo/config` if you want to apply the setting globaly) with the `linker`
value.

```toml
[target.arm-linux-androideabi]
linker = "<path-to-your-toolchain>/android-18-toolchain/bin/arm-linux-androideabi-gcc"
ar = "<path-to-your-toolchain>/android-18-toolchain/bin/arm-linux-androideabi-ar"
```

This crate indirectly depends on
[libgstreamer_android_gen](https://github.com/servo/libgstreamer_android_gen):
a tool to generate the required `libgstreamer_android.so` library with all
GStreamer dependencies for Android and some Java code required to initialize
GStreamer on Android.

The final step requires fetching or generating this dependency and setting the pkg-config to use
`libgstreamer_android.so`. To do that, there's a [helper script](etc/android_bootstrap.sh)
that will fetch the latest version of this dependency generated for
Servo. To run the script do:

```
cd etc
./android_bootstrap.sh <target>
```

where `target` can be `armeabi-v7` or `x86`.

After running the script, you will need to add the path to the `pkg-config`
info for all GStreamer dependencies to your `PKG_CONFIG_PATH` environment variable
The script will output the path and a command suggestion. For example:

```
export PKG_CONFIG_PATH=/Users/ferjm/dev/mozilla/media/etc/../gstreamer/armeabi-v7a/gst-build-armeabi-v7a/pkgconfig
```

If you want to generate your own `libgstreamer_android.so`
bundle, check the documentation from that repo and tweak the
[helper script](https://github.com/servo/media/blob/a9c73680eef72d48f975df55fe9451020e350fad/etc/android_bootstrap.sh#L24) accordingly.

## Build
For macOS, Windows, and Linux, simply run:
```bash
cargo build
```
To also build the xamples:
```bash
cargo build --all
```
For Android, run:
```bash
PKG_CONFIG_ALLOW_CROSS=1 cargo build --target=arm-linux-androideabi
```

## Running the examples
### Android
Make sure that you have [adb](https://developer.android.com/studio/command-line/adb)
installed and you have adb access to your
Android device. Go to the `examples/android` folder and run:
```ssh
source build.sh
./run.sh
```
