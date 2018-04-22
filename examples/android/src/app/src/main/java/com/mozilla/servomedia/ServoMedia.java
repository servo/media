package com.mozilla.servomedia;

import java.io.Closeable;
import android.content.Context;
import org.freedesktop.gstreamer.GStreamer;

public class ServoMedia {

    public static void init(Context context) throws Exception {
        System.loadLibrary("gstreamer_android");
        GStreamer.init(context);

        System.loadLibrary("servo_media_android");
      }

    private static native String backendId();
    private static native void testStream();

    public String getBackendId() {
        return backendId();
    }

    public void playStream() {
        testStream();
    }
}
