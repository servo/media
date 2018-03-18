ROOT=${PWD}
PKG_CONFIG_PATH_OLD=$PKG_CONFIG_PATH
cd ../../
export PKG_CONFIG_PATH=${PWD}/target/gst-build-armeabi/pkgconfig
cd ${ROOT}
cd lib
PKG_CONFIG_ALLOW_CROSS=1 cargo build
export PKG_CONFIG_PATH=$PKG_CONFIG_PATH_OLD
cd ../src/app/src/main/

mkdir jniLibs 2>&1 >/dev/null
mkdir jniLibs/armeabi 2>&1 >/dev/null

ln -s ${ROOT}/lib/target/arm-linux-androideabi/debug/libservo_media_android.so jniLibs/armeabi/libservo_media_android.so 2>&1 >/dev/null
ln -s ${ROOT}/../../target/gst-build-armeabi/libgstreamer_android.so jniLibs/armeabi/libgstreamer_android.so 2>&1 >/dev/null

cd ${ROOT}/src
./gradlew installDebug
adb shell am start -n com.mozilla.servomedia/.MainActivity
adb logcat | egrep '(servomedia)'
