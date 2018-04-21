wget https://github.com/ferjm/libgstreamer_android_gen/blob/gst1.14/out/src.zip?raw=true -O src.zip
unzip src.zip -d src_
mv src_/src/org src/app/src/main/java/
mv src_/src/main/* src/app/src/main/
rm -rf src.zip
rm -rf src_
