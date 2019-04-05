extern crate gstreamer as gst;
extern crate gstreamer_video as gst_video;

extern crate servo_media_player as sm_player;

pub trait Render {
    fn is_gl(&self) -> bool;

    fn build_frame(
        &self,
        buffer: gst::Buffer,
        info: gst_video::VideoInfo,
    ) -> Result<sm_player::frame::Frame, ()>;

    fn build_video_sink(
        &self,
        appsink: &gst::Element,
        pipeline: &gst::Element,
    ) -> Result<(), sm_player::PlayerError>;
}
