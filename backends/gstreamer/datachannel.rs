use glib::{ObjectExt, Value};
use servo_media_webrtc::thread::InternalEvent;
use servo_media_webrtc::WebRtcController as WebRtcThread;
use servo_media_webrtc::{
    DataChannelEvent, DataChannelId, DataChannelInit, DataChannelMessage, WebRtcError, WebRtcResult,
};
use std::sync::Mutex;

// XXX Most of this code will be outdated once
//     https://gitlab.freedesktop.org/gstreamer/gst-plugins-bad/-/issues/1168
//     is fixed.

#[derive(Clone)]
struct DataChannel(glib::Object);

impl DataChannel {
    pub fn send(&self, message: &DataChannelMessage) -> WebRtcResult {
        match message {
            DataChannelMessage::Text(message) => {
                self.0.emit("send-string", &[&Value::from(&message)])
            }
            DataChannelMessage::Binary(message) => {
                let bytes = glib::Bytes::from(message);
                self.0.emit("send-data", &[&Value::from(&bytes)])
            }
        }
        .map(|_| ())
        .map_err(|e| WebRtcError::Backend(e.to_string()))
    }

    pub fn close(&self) -> WebRtcResult {
        self.0
            .emit("close", &[])
            .map(|_| ())
            .map_err(|e| WebRtcError::Backend(e.to_string()))
    }
}

// The datachannel object is thread-safe
unsafe impl Send for DataChannel {}
unsafe impl Sync for DataChannel {}

pub struct GStreamerWebRtcDataChannel {
    channel: DataChannel,
    id: DataChannelId,
    thread: WebRtcThread,
}

impl GStreamerWebRtcDataChannel {
    pub fn new(
        id: &DataChannelId,
        webrtc: &gst::Element,
        thread: &WebRtcThread,
        init: &DataChannelInit,
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

        GStreamerWebRtcDataChannel::from(id, channel, thread)
    }

    pub fn from(
        id: &DataChannelId,
        channel: glib::Object,
        thread: &WebRtcThread,
    ) -> Result<Self, String> {
        let id_ = id.clone();
        let thread_ = Mutex::new(thread.clone());
        channel
            .connect("on-open", false, move |_| {
                thread_
                    .lock()
                    .unwrap()
                    .internal_event(InternalEvent::OnDataChannelEvent(
                        id_,
                        DataChannelEvent::Open,
                    ));
                None
            })
            .map_err(|e| e.to_string())?;

        let id_ = id.clone();
        let thread_ = Mutex::new(thread.clone());
        channel
            .connect("on-close", false, move |_| {
                thread_
                    .lock()
                    .unwrap()
                    .internal_event(InternalEvent::OnDataChannelEvent(
                        id_,
                        DataChannelEvent::Close,
                    ));
                None
            })
            .map_err(|e| e.to_string())?;

        let id_ = id.clone();
        let thread_ = Mutex::new(thread.clone());
        channel
            .connect("on-error", false, move |error| {
                if let Some(error) = error[0]
                    .get::<glib::error::Error>()
                    .expect("Invalid GError")
                {
                    thread_
                        .lock()
                        .unwrap()
                        .internal_event(InternalEvent::OnDataChannelEvent(
                            id_,
                            DataChannelEvent::Error(WebRtcError::Backend(error.to_string())),
                        ));
                }
                None
            })
            .map_err(|e| e.to_string())?;

        let id_ = id.clone();
        let thread_ = Mutex::new(thread.clone());
        channel
            .connect("on-message-string", false, move |message| {
                if let Some(message) = message[1]
                    .get::<String>()
                    .expect("Invalid data channel message")
                {
                    thread_
                        .lock()
                        .unwrap()
                        .internal_event(InternalEvent::OnDataChannelEvent(
                            id_,
                            DataChannelEvent::OnMessage(message),
                        ));
                }
                None
            })
            .map_err(|e| e.to_string())?;

        Ok(Self {
            id: id.clone(),
            thread: thread.clone(),
            channel: DataChannel(channel),
        })
    }

    pub fn send(&self, message: &DataChannelMessage) -> WebRtcResult {
        self.channel.send(message)
    }

    pub fn close(&self) -> WebRtcResult {
        self.channel.close()
    }
}

impl Drop for GStreamerWebRtcDataChannel {
    fn drop(&mut self) {
        self.thread
            .internal_event(InternalEvent::OnDataChannelEvent(
                self.id,
                DataChannelEvent::Close,
            ));
    }
}
