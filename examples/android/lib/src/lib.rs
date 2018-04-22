extern crate servo_media;

use servo_media::ServoMedia;
use std::os::raw::c_char;
use std::ffi::CString;

#[no_mangle]
pub extern "C" fn servo_media_backend_id() -> *mut c_char {
    match ServoMedia::get() {
        Ok(servo_media) => CString::new(servo_media.version()).unwrap().into_raw(),
        Err(_) => CString::new("Ooops").unwrap().into_raw(),
    }
}

#[no_mangle]
pub extern "C" fn servo_media_test_stream() {
    match ServoMedia::get() {
        Ok(servo_media) => {
            match servo_media.get_audio_stream() {
                Ok(stream) => {
                    stream.play();
                    // FIXME: return stream and store it in JNI env to prevent GC to kick in.
                }
                Err(_) => {}
            };
        }
        Err(_) => {}
    };
}

/// Expose the JNI interface for android below
#[cfg(target_os = "android")]
#[allow(non_snake_case)]
pub mod android {
    extern crate jni;

    use super::*;
    use self::jni::JNIEnv;
    use self::jni::objects::JClass;
    use self::jni::sys::jstring;

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
    pub unsafe extern "C" fn Java_com_mozilla_servomedia_ServoMedia_testStream(
        _env: JNIEnv,
        _: JClass,
    ) {
        servo_media_test_stream();
    }
}

#[test]
fn it_works() {
    let backend_id = unsafe { CString::from_raw(servo_media_backend_id()) };
    assert_eq!(backend_id.to_str().unwrap(), "GStreamer");
}
