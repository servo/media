ROOT=${PWD}
cd src/app/src/main/

rm -rf jniLibs
mkdir -p jniLibs/armeabi

ln -s ${ROOT}/../../target/arm-linux-androideabi/debug/libservo_media_android.so jniLibs/armeabi/libservo_media_android.so
ln -s ${ROOT}/../../gstreamer/armeabi-v7a/gst-build-armeabi-v7a/libgstreamer_android.so jniLibs/armeabi/libgstreamer_android.so

cd ${ROOT}/src
./gradlew installDebug || return 1

cd ${ROOT}

adb shell am start -n com.mozilla.servomedia/.MainActivity
adb logcat | egrep '(servo|gst)'
