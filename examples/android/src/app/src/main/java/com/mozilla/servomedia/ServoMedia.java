package com.mozilla.servomedia;

import java.io.Closeable;
import android.content.Context;
import android.os.SystemClock;
import org.freedesktop.gstreamer.GStreamer;

public class ServoMedia {
    public static void init(Context context) throws Exception {
        System.loadLibrary("gstreamer_android");
        GStreamer.init(context);

        System.loadLibrary("servo_media_android");
      }

    private static native String backendId();
    private static native long audioStreamNew();
    private static native void audioStreamPlay(long ptr);
    private static native void audioStreamStop(long ptr);
    private static native void audioStreamRelease(long ptr);

    public String getBackendId() {
        return backendId();
    }

    public void playStream() {
      long streamPtr = audioStreamNew();
      audioStreamPlay(streamPtr);
    }
}
