use boxfnonce::SendBoxFnOnce;
use glib::{ObjectExt, Value};
use servo_media_webrtc::thread::InternalEvent;
use servo_media_webrtc::WebRtcController as WebRtcThread;
use servo_media_webrtc::{
    InnerWebRtcDataChannel, WebRtcDataChannelBackend, WebRtcDataChannelCallbacks,
    WebRtcDataChannelInit, WebRtcError, WebRtcResult,
};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct InnerDataChannel {
    channel: glib::SendWeakRef<glib::Object>,
}

impl InnerWebRtcDataChannel for InnerDataChannel {
    fn send(&self, message: &str) -> WebRtcResult {
        if let Some(channel) = self.channel.upgrade() {
            channel
                .emit("send-string", &[&Value::from(message)])
                .map(|_| ())
                .map_err(|e| WebRtcError::Backend(e.to_string()))
        } else {
            Err(WebRtcError::Backend("Dropped channel".to_owned()))
        }
    }
}

pub struct GStreamerWebRtcDataChannel {
    channel: InnerDataChannel,
    webrtc_thread: WebRtcThread,
    callbacks: Arc<Mutex<WebRtcDataChannelCallbacks>>,
}

impl GStreamerWebRtcDataChannel {
    pub fn new(
        webrtc_thread: WebRtcThread,
        webrtc: &gst::Element,
        init: &WebRtcDataChannelInit,
    ) -> Result<Self, String> {
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

        GStreamerWebRtcDataChannel::from(channel, webrtc_thread)
    }

    pub fn from(channel: glib::Object, webrtc_thread: WebRtcThread) -> Result <Self, String> {
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

        let channel = InnerDataChannel {
            channel: glib::SendWeakRef::from(channel.downgrade()),
        };
        Ok(Self {
            webrtc_thread,
            channel,
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

    fn set_on_error(&self, cb: Box<dyn FnOnce(WebRtcError,) + Send + 'static>) {
        self.callbacks.lock().unwrap().error = Some(SendBoxFnOnce::from(cb));
    }

    fn set_on_message(&self, cb: Box<dyn Fn(String) + Send + 'static>) {
        self.callbacks.lock().unwrap().message = Some(cb);
    }

    fn send(&self, message: &str) -> WebRtcResult {
        // glib::object::SendWeakRef needs to be upgraded from the thread
        // where it was created. In this case, the channel weak ref
        // was created on the webrtc controller's thread.
        self.webrtc_thread
            .internal_event(InternalEvent::SendDataChannelMessage(
                Box::new(self.channel.clone()),
                message.to_owned(),
            ));
        Ok(())
    }

    fn close(&self) {}
}
