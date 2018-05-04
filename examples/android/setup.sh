wget https://github.com/ferjm/libgstreamer_android_gen/blob/gst1.14/out/src.zip?raw=true -O src.zip
unzip src.zip -d src_

cp -v src_/src/org/freedesktop/gstreamer/GStreamer.java src/app/src/main/java/org/freedesktop/gstreamer/
cp -rv src_/src/main/* src/app/src/main/

rm -rf src.zip
rm -rf src_
