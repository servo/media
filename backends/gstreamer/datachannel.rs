use boxfnonce::SendBoxFnOnce;
use glib::{ObjectExt, Value};
use servo_media_webrtc::{
    WebRtcDataChannelBackend, WebRtcDataChannelCallbacks, WebRtcDataChannelInit, WebRtcError,
    WebRtcResult,
};
use std::sync::{Arc, Mutex};

// XXX Most of this code will be outdated once
//     https://gitlab.freedesktop.org/gstreamer/gst-plugins-bad/-/issues/1168
//     is fixed.

#[derive(Clone)]
struct DataChannel(glib::Object);

impl DataChannel {
    pub fn send(&self, message: &str) -> WebRtcResult {
        self.0
            .emit("send-string", &[&Value::from(message)])
            .map(|_| ())
            .map_err(|e| WebRtcError::Backend(e.to_string()))
    }
}

// The datachannel object is thread-safe
unsafe impl Send for DataChannel {}
unsafe impl Sync for DataChannel {}

pub struct GStreamerWebRtcDataChannel {
    channel: DataChannel,
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

        GStreamerWebRtcDataChannel::from(channel)
    }

    pub fn from(channel: glib::Object) -> Result<Self, String> {
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
                        .error(WebRtcError::Backend(error.to_string()));
                }
                None
            })
            .map_err(|e| e.to_string())?;

        let callbacks_ = callbacks.clone();
        channel
            .connect("on-message-string", false, move |message| {
                if let Some(message) = message[1]
                    .get::<String>()
                    .expect("Invalid data channel message")
                {
                    callbacks_.lock().unwrap().message(message);
                }
                None
            })
            .map_err(|e| e.to_string())?;

        Ok(Self {
            channel: DataChannel(channel),
            callbacks,
        })
    }
}

impl WebRtcDataChannelBackend for GStreamerWebRtcDataChannel {
    fn set_on_open(&self, cb: Box<dyn FnOnce() + Send + 'static>) {
        self.callbacks.lock().unwrap().open = Some(SendBoxFnOnce::from(cb));
    }
    fn set_on_close(&self, cb: Box<dyn FnOnce() + Send + 'static>) {
        self.callbacks.lock().unwrap().close = Some(SendBoxFnOnce::from(cb));
    }

    fn set_on_error(&self, cb: Box<dyn FnOnce(WebRtcError) + Send + 'static>) {
        self.callbacks.lock().unwrap().error = Some(SendBoxFnOnce::from(cb));
    }

    fn set_on_message(&self, cb: Box<dyn Fn(String) + Send + 'static>) {
        self.callbacks.lock().unwrap().message = Some(cb);
    }

    fn send(&self, message: &str) -> WebRtcResult {
        self.channel.send(message)
    }

    fn close(&self) {}
}
