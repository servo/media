package com.mozilla.servomedia;

import android.support.v7.app.AppCompatActivity;
import android.os.Bundle;
import android.widget.TextView;

public class MainActivity extends AppCompatActivity {

    static {
        System.loadLibrary("servo_media_android");
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        setContentView(R.layout.activity_main);

        ServoMedia media = new ServoMedia();
        String backendId = media.getBackendId();
        ((TextView)findViewById(R.id.backendId)).setText(backendId);

        media.playStream();
    }
}
