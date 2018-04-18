package com.mozilla.servomedia;

public class ServoMedia {

    private static native String backendId();
    private static native void testStream();

    public String getBackendId() {
        return backendId();
    }

    public void playStream() {
        testStream();
    }
}
