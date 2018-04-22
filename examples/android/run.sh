ROOT=${PWD}
PKG_CONFIG_PATH_OLD=$PKG_CONFIG_PATH
cd ../../
export PKG_CONFIG_PATH=${PWD}/target/gst-build-armeabi/pkgconfig
cd ${ROOT}/lib
PKG_CONFIG_ALLOW_CROSS=1 cargo build || exit 1
export PKG_CONFIG_PATH=$PKG_CONFIG_PATH_OLD
cd ../src/app/src/main/

rm -rf jniLibs
mkdir -p jniLibs/armeabi

ln -s ${ROOT}/lib/target/arm-linux-androideabi/debug/libservo_media_android.so jniLibs/armeabi/libservo_media_android.so
ln -s ${ROOT}/../../target/gst-build-armeabi/libgstreamer_android.so jniLibs/armeabi/libgstreamer_android.so

cd ${ROOT}/src
./gradlew installDebug || exit 1

adb shell am start -n com.mozilla.servomedia/.MainActivity
adb logcat | egrep '(servo|gst)'
