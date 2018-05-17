extern crate servo_media;

use servo_media::ServoMedia;
use std::ffi::CString;
use std::os::raw::c_char;

struct AudioStream {
    inner: servo_media::AudioGraph,
}

impl AudioStream {
    pub fn new() -> Self {
        Self {
            inner: ServoMedia::get().unwrap().create_audio_graph().unwrap(),
        }
    }

    pub fn play(&self) {
        self.inner.resume_processing()
    }

    pub fn stop(&self) {
        self.inner.pause_processing()
    }
}

#[no_mangle]
pub extern "C" fn servo_media_backend_id() -> *mut c_char {
    match ServoMedia::get() {
        Ok(servo_media) => CString::new(servo_media.version()).unwrap().into_raw(),
        Err(_) => CString::new("Ooops").unwrap().into_raw(),
    }
}

/// Expose the JNI interface for android below
#[cfg(target_os = "android")]
#[allow(non_snake_case)]
pub mod android {
    extern crate jni;

    use self::jni::objects::JClass;
    use self::jni::sys::{jlong, jstring};
    use self::jni::JNIEnv;
    use super::*;

    #[no_mangle]
    pub unsafe extern "C" fn Java_com_mozilla_servomedia_ServoMedia_backendId(
        env: JNIEnv,
        _: JClass,
    ) -> jstring {
        let backend_id = CString::from_raw(servo_media_backend_id());
        let output = env.new_string(backend_id.to_str().unwrap())
            .expect("Couldn't create java string!");

        output.into_inner()
    }

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
