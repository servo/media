use boxfnonce::SendBoxFnOnce;
use glib::{ObjectExt, Value};
use servo_media_webrtc::{
    WebRtcDataChannel, WebRtcDataChannelCallbacks, WebRtcDataChannelInit, WebrtcError, WebrtcResult,
};
use std::sync::{Arc, Mutex};

pub struct GStreamerWebRtcDataChannel {
    channel: glib::SendWeakRef<glib::Object>,
    callbacks: Arc<Mutex<WebRtcDataChannelCallbacks>>,
}

impl GStreamerWebRtcDataChannel {
    pub fn new(webrtc: &gst::Element, init: &WebRtcDataChannelInit) -> Result<Self, String> {
        let channel = webrtc
            .emit(
                "create-data-channel",
                &[&init.label, &None::<gst::Structure>],
            )
            .map_err(|e| e.to_string())?;
        let channel = channel
            .expect("Invalid datachannel")
            .get::<glib::Object>()
            .map_err(|e| e.to_string())?
            .expect("Invalid datachannel");

        let callbacks = Arc::new(Mutex::new(WebRtcDataChannelCallbacks::new()));

        let callbacks_ = callbacks.clone();
        channel
            .connect("on-open", false, move |_| {
                callbacks_.lock().unwrap().open();
                None
            })
            .map_err(|e| e.to_string())?;

        let callbacks_ = callbacks.clone();
        channel
            .connect("on-close", false, move |_| {
                callbacks_.lock().unwrap().close();
                None
            })
            .map_err(|e| e.to_string())?;

        let callbacks_ = callbacks.clone();
        channel
            .connect("on-error", false, move |error| {
                if let Some(error) = error[0]
                    .get::<glib::error::Error>()
                    .expect("Invalid GError")
                {
                    callbacks_
                        .lock()
                        .unwrap()
                        .error(WebrtcError::Backend(error.to_string()));
                }
                None
            })
            .map_err(|e| e.to_string())?;

        let callbacks_ = callbacks.clone();
        channel
            .connect("on-message-string", false, move |message| {
                if let Some(message) = message[0]
                    .get::<String>()
                    .expect("Invalid data channel message")
                {
                    println!("GOT MESSAGE {:?}", message);
                    callbacks_.lock().unwrap().message(message);
                }
                None
            })
            .map_err(|e| e.to_string())?;

        let channel = glib::SendWeakRef::from(channel.downgrade());
        Ok(Self { channel, callbacks })
    }
}

impl WebRtcDataChannel for GStreamerWebRtcDataChannel {
    fn set_on_open(&self, cb: SendBoxFnOnce<'static, ()>) {
        self.callbacks.lock().unwrap().open = Some(cb);
    }

    fn set_on_close(&self, cb: SendBoxFnOnce<'static, ()>) {
        self.callbacks.lock().unwrap().close = Some(cb);
    }

    fn set_on_error(&self, cb: SendBoxFnOnce<'static, (WebrtcError,)>) {
        self.callbacks.lock().unwrap().error = Some(cb);
    }

    fn set_on_message(&self, cb: Box<dyn Fn(String) + Send + Sync + 'static>) {
        self.callbacks.lock().unwrap().message = Some(cb);
    }

    fn send(&self, message: &str) -> WebrtcResult {
        if let Some(channel) = self.channel.upgrade() {
            channel
                .emit("send-string", &[&Value::from(message)])
                .map(|_| ())
                .map_err(|e| WebrtcError::Backend(e.to_string()))
        } else {
            Err(WebrtcError::Backend("Dropped channel".to_owned()))
        }
    }

    fn close(&self) {}
}
