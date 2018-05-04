package com.mozilla.servomedia;

import android.content.Context;
import android.os.Bundle;
import android.os.PowerManager;

import android.view.View;
import android.view.View.OnClickListener;
import android.widget.ImageButton;
import android.widget.TextView;
import android.widget.Toast;
import android.support.v7.app.AppCompatActivity;

public class MainActivity extends AppCompatActivity {
    private PowerManager.WakeLock wake_lock;
    private ServoMedia media;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);

        try {
            ServoMedia.init(this);
        } catch (Exception e) {
            Toast.makeText(this, e.getMessage(), Toast.LENGTH_LONG).show();
            finish();
            return;
        }

        setContentView(R.layout.activity_main);

        media = new ServoMedia();

        PowerManager pm = (PowerManager) getSystemService(Context.POWER_SERVICE);
        wake_lock = pm.newWakeLock(PowerManager.FULL_WAKE_LOCK, "GStreamer Play");
        wake_lock.setReferenceCounted(false);

        ImageButton play = (ImageButton) this.findViewById(R.id.button_play);
        play.setOnClickListener(new OnClickListener() {
                public void onClick(View v) {
                    media.playStream();
                    wake_lock.acquire();
                }
            });

        ImageButton pause = (ImageButton) this.findViewById(R.id.button_pause);
        pause.setOnClickListener(new OnClickListener() {
                public void onClick(View v) {
                    media.stopStream();
                    wake_lock.release();
                }
            });

        String backendId = media.getBackendId();
        ((TextView)findViewById(R.id.backendId)).setText(backendId);

    }
}
