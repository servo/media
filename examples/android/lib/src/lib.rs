extern crate servo_media;

use servo_media::audio::gain_node::GainNodeOptions;
use servo_media::audio::context::AudioContext;
use servo_media::audio::node::{AudioNodeInit, AudioNodeMessage};
use servo_media::audio::node::AudioScheduledSourceNodeMessage;
use servo_media::{Backend, ServoMedia};

struct AudioStream {
    context: AudioContext<Backend>,
}

impl AudioStream {
    pub fn new() -> Self {
        let context = ServoMedia::get().unwrap().create_audio_context(Default::default());
        let osc = context.create_node(AudioNodeInit::OscillatorNode(Default::default()));
        let mut options = GainNodeOptions::default();
        options.gain = 0.5;
        let gain = context.create_node(AudioNodeInit::GainNode(options));
        let dest = context.dest_node();
        context.connect_ports(osc.output(0), gain.input(0));
        context.connect_ports(gain.output(0), dest.input(0));
        context.message_node(
            osc,
            AudioNodeMessage::AudioScheduledSourceNode(AudioScheduledSourceNodeMessage::Start(0.)),
        );
        Self { context }
    }

    pub fn play(&mut self) {
        let _ = self.context.resume();
    }

    pub fn stop(&mut self) {
        let _ = self.context.suspend();
    }
}

/// Expose the JNI interface for android below
#[cfg(target_os = "android")]
#[allow(non_snake_case)]
pub mod android {
    extern crate jni;

    use self::jni::objects::JClass;
    use self::jni::sys::jlong;
    use self::jni::JNIEnv;
    use super::*;

    #[no_mangle]
    pub unsafe extern "C" fn Java_com_mozilla_servomedia_ServoMedia_audioStreamNew(
        _: JNIEnv,
        _: JClass,
    ) -> jlong {
        let stream = AudioStream::new();
        Box::into_raw(Box::new(stream)) as jlong
    }

    #[no_mangle]
    pub unsafe extern "C" fn Java_com_mozilla_servomedia_ServoMedia_audioStreamPlay(
        _: JNIEnv,
        _: JClass,
        stream_ptr: jlong,
    ) {
        let stream = &mut *(stream_ptr as *mut AudioStream);
        stream.play();
    }

    #[no_mangle]
    pub unsafe extern "C" fn Java_com_mozilla_servomedia_ServoMedia_audioStreamStop(
        _: JNIEnv,
        _: JClass,
        stream_ptr: jlong,
    ) {
        let stream = &mut *(stream_ptr as *mut AudioStream);
        stream.stop();
    }

    #[no_mangle]
    pub unsafe extern "C" fn Java_com_mozilla_servomedia_ServoMedia_audioStreamDestroy(
        _: JNIEnv,
        _: JClass,
        stream_ptr: jlong,
    ) {
        let _ = Box::from_raw(stream_ptr as *mut AudioStream);
    }
}

#[test]
fn it_works() {
    let backend_id = unsafe { CString::from_raw(servo_media_backend_id()) };
    assert_eq!(backend_id.to_str().unwrap(), "GStreamer");
}
