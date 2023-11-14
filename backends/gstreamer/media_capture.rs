use crate::media_stream::GStreamerMediaStream;
use gst;
use gst::caps::NoFeature;
use gst::prelude::*;
use servo_media_streams::capture::*;
use servo_media_streams::registry::MediaStreamId;
use servo_media_streams::MediaStreamType;
use std::i32;

trait AddToCaps {
    type Bound;
    fn add_to_caps(
        &self,
        name: &str,
        min: Self::Bound,
        max: Self::Bound,
        builder: gst::caps::Builder<NoFeature>,
    ) -> Option<gst::caps::Builder<NoFeature>>;
}

impl AddToCaps for Constrain<u32> {
    type Bound = u32;
    fn add_to_caps(
        &self,
        name: &str,
        min: u32,
        max: u32,
        builder: gst::caps::Builder<NoFeature>,
    ) -> Option<gst::caps::Builder<NoFeature>> {
        match self {
            Constrain::Value(v) => Some(builder.field(name, v)),
            Constrain::Range(r) => {
                let min = into_i32(r.min.unwrap_or(min));
                let max = into_i32(r.max.unwrap_or(max));
                let range = gst::IntRange::<i32>::new(min, max);

                // TODO: Include the ideal caps value in the caps, needs a refactor
                //       of the AddToCaps trait
                Some(builder.field(name, range))
            }
        }
    }
}

fn into_i32(x: u32) -> i32 {
    if x > i32::MAX as u32 {
        i32::MAX
    } else {
        x as i32
    }
}

impl AddToCaps for Constrain<f64> {
    type Bound = i32;
    fn add_to_caps<'a>(
        &self,
        name: &str,
        min: i32,
        max: i32,
        builder: gst::caps::Builder<NoFeature>,
    ) -> Option<gst::caps::Builder<NoFeature>> {
        match self {
            Constrain::Value(v) => Some(builder.field("name", gst::Fraction::approximate_f64(*v)?)),
            Constrain::Range(r) => {
                let min = r
                    .min
                    .and_then(gst::Fraction::approximate_f64)
                    .unwrap_or(gst::Fraction::new(min, 1));
                let max = r
                    .max
                    .and_then(gst::Fraction::approximate_f64)
                    .unwrap_or(gst::Fraction::new(max, 1));
                let range = gst::FractionRange::new(min, max);
                // TODO: Include the ideal caps value in the caps, needs a refactor
                //       of the AddToCaps trait
                Some(builder.field(name, range))
            }
        }
    }
}

// TODO(Manishearth): Should support a set of constraints
fn into_caps(set: MediaTrackConstraintSet, format: &str) -> Option<gst::Caps> {
    let mut builder = gst::Caps::builder(format);
    if let Some(w) = set.width {
        builder = w.add_to_caps("width", 0, 1000000, builder)?;
    }
    if let Some(h) = set.height {
        builder = h.add_to_caps("height", 0, 1000000, builder)?;
    }
    if let Some(aspect) = set.aspect {
        builder = aspect.add_to_caps("pixel-aspect-ratio", 0, 1000000, builder)?;
    }
    if let Some(fr) = set.frame_rate {
        builder = fr.add_to_caps("framerate", 0, 1000000, builder)?;
    }
    if let Some(sr) = set.sample_rate {
        builder = sr.add_to_caps("rate", 0, 1000000, builder)?;
    }
    Some(builder.build())
}

struct GstMediaDevices {
    monitor: gst::DeviceMonitor,
}

impl GstMediaDevices {
    pub fn new() -> Self {
        Self {
            monitor: gst::DeviceMonitor::new(),
        }
    }

    pub fn get_track(
        &self,
        video: bool,
        constraints: MediaTrackConstraintSet,
    ) -> Option<GstMediaTrack> {
        let (format, filter) = if video {
            ("video/x-raw", "Video/Source")
        } else {
            ("audio/x-raw", "Audio/Source")
        };
        let caps = into_caps(constraints, format)?;
        let f = self.monitor.add_filter(Some(filter), Some(&caps));
        let devices = self.monitor.devices();
        if let Some(f) = f {
            let _ = self.monitor.remove_filter(f);
        }
        if let Some(d) = devices.front() {
            let element = d.create_element(None).ok()?;
            Some(GstMediaTrack { element })
        } else {
            None
        }
    }
}

pub struct GstMediaTrack {
    element: gst::Element,
}

fn create_input_stream(
    stream_type: MediaStreamType,
    constraint_set: MediaTrackConstraintSet,
) -> Option<MediaStreamId> {
    let devices = GstMediaDevices::new();
    devices
        .get_track(stream_type == MediaStreamType::Video, constraint_set)
        .map(|track| {
            let f = match stream_type {
                MediaStreamType::Audio => GStreamerMediaStream::create_audio_from,
                MediaStreamType::Video => GStreamerMediaStream::create_video_from,
            };
            f(track.element)
        })
}

pub fn create_audioinput_stream(constraint_set: MediaTrackConstraintSet) -> Option<MediaStreamId> {
    create_input_stream(MediaStreamType::Audio, constraint_set)
}

pub fn create_videoinput_stream(constraint_set: MediaTrackConstraintSet) -> Option<MediaStreamId> {
    create_input_stream(MediaStreamType::Video, constraint_set)
}
