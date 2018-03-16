ROOT=${PWD}
cd lib
cargo build
cd ../src/app/src/main/

mkdir jniLibs 2>&1 >/dev/null
mkdir jniLibs/armeabi 2>&1 >/dev/null

ln -s ${ROOT}/lib/target/arm-linux-androideabi/debug/libservo_media_android.so jniLibs/armeabi/libservo_media_android.so 2>&1 >/dev/null

cd ${ROOT}/src
./gradlew installDebug
adb shell am start -n com.mozilla.servomedia/.MainActivity
adb logcat | egrep '(servomedia)'
