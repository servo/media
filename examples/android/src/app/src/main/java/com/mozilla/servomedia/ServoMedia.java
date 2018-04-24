package com.mozilla.servomedia;

import java.io.Closeable;
import android.content.Context;
import android.os.SystemClock;
import org.freedesktop.gstreamer.GStreamer;

public class ServoMedia {
    private long streamPtr;

    public static void init(Context context) throws Exception {
        System.loadLibrary("gstreamer_android");
        GStreamer.init(context);

        System.loadLibrary("servo_media_android");
      }

    protected ServoMedia() {
        this.streamPtr = audioStreamNew();
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
        audioStreamPlay(this.streamPtr);
    }

    public void stopStream() {
        audioStreamStop(this.streamPtr);
    }
}
