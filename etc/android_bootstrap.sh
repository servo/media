set -e

if [ "$#" -ne 1 ]; then
  echo "Usage: ./android_bootstrap <target> (supported targets: armeabi-v7a x86)" >&2
  exit 1
fi

TARGET=$1

if [ $TARGET != "armeabi-v7a" ] && [ $TARGET != "x86" ]; then
  echo "Unsupported target (supported targets: armeabi-v7a x86)" >&2
  exit 1
fi

GST_DIR="${PWD}/../gstreamer"
if [ ! -d $GST_DIR ]; then
  mkdir $GST_DIR
fi

GST_DIR_TARGET="${GST_DIR}/${TARGET}"
if [ ! -d $GST_DIR_TARGET ]; then
  GST_ZIP="../gstreamer-${TARGET}.zip"
  # Download the bundle containing all dependencies for Android.
  wget https://servo-deps.s3.amazonaws.com/gstreamer/gstreamer-$TARGET-1.14.3-20181004-142930.zip -O $GST_ZIP
  unzip $GST_ZIP -d $GST_DIR_TARGET > /dev/null
  rm $GST_ZIP
fi

GST_LIB_DIR="${GST_DIR}/${TARGET}/gst-build-${TARGET}"
# Fix pkg-config info to point to the location of the libgstreamer_android.so lib
perl -i -pe "s#libdir=.*#libdir=${GST_LIB_DIR}#g" $GST_LIB_DIR/pkgconfig/*

echo "\n\nYou need to add ${GST_LIB_DIR}/pkgconfig to your PKG_CONFIG_PATH.\n\n" \
"i.e. export PKG_CONFIG_PATH=${GST_LIB_DIR}/pkgconfig"
