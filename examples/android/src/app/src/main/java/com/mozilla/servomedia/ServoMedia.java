package com.mozilla.servomedia;

public class ServoMedia {

    private static native String backendId();

    public String getBackendId() {
        return backendId();
    }
}
