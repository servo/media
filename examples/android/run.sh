ROOT=${PWD}
PKG_CONFIG_PATH_OLD=$PKG_CONFIG_PATH
cd ../../
export PKG_CONFIG_PATH=${PWD}/backends/gstreamer/target/gst-build-armeabi/pkgconfig
echo "Set PKG_CONFIG_PATH to ${PKG_CONFIG_PATH}"
cd servo-media
echo "Building servo-media ${PWD}"
PKG_CONFIG_ALLOW_CROSS=1 cargo build --target=arm-linux-androideabi || return 1
cd ${ROOT}/lib
echo "Building servo-media-android ${PWD}"
PKG_CONFIG_ALLOW_CROSS=1 cargo build || return 1
echo "Set PKG_CONFIG_PATH to previous state ${PKG_CONFIG_PATH_OLD}"
export PKG_CONFIG_PATH=$PKG_CONFIG_PATH_OLD
cd ../src/app/src/main/

rm -rf jniLibs
mkdir -p jniLibs/armeabi

ln -s ${ROOT}/../../target/arm-linux-androideabi/debug/libservo_media_android.so jniLibs/armeabi/libservo_media_android.so
ln -s ${ROOT}/../../backends/gstreamer/target/gst-build-armeabi/libgstreamer_android.so jniLibs/armeabi/libgstreamer_android.so

cd ${ROOT}/src
./gradlew installDebug || return 1

cd ${ROOT}

adb shell am start -n com.mozilla.servomedia/.MainActivity
adb logcat | egrep '(servo|gst)'
