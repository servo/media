ROOT=${PWD}
PKG_CONFIG_PATH_OLD=$PKG_CONFIG_PATH
cd ../../etc
./android_bootstrap.sh armeabi-v7a
cd ..
export PKG_CONFIG_PATH=${PWD}/gstreamer/armeabi-v7a/gst-build-armeabi-v7a/pkgconfig
echo "Set PKG_CONFIG_PATH to ${PKG_CONFIG_PATH}"
cd servo-media
echo "Building servo-media ${PWD}"
PKG_CONFIG_ALLOW_CROSS=1 cargo build --target=arm-linux-androideabi || return 1
cd ${ROOT}/lib
echo "Building servo-media-android ${PWD}"
PKG_CONFIG_ALLOW_CROSS=1 cargo build --target=arm-linux-androideabi || return 1
echo "Set PKG_CONFIG_PATH to previous state ${PKG_CONFIG_PATH_OLD}"
export PKG_CONFIG_PATH=$PKG_CONFIG_PATH_OLD
