use gst::Caps;
use gst::DeviceExt;
use gst::DeviceMonitor as GstDeviceMonitor;
use gst::DeviceMonitorExt;
use gst::DeviceMonitorExtManual;
use std::cell::RefCell;

use servo_media_streams::device_monitor::{MediaDeviceInfo, MediaDeviceKind, MediaDeviceMonitor};

pub struct GStreamerDeviceMonitor {
    devices: RefCell<Option<Vec<MediaDeviceInfo>>>,
}

impl GStreamerDeviceMonitor {
    pub fn new() -> Self {
        Self {
            devices: RefCell::new(None),
        }
    }

    fn get_devices(&self) -> Result<Vec<MediaDeviceInfo>, ()> {
        const AUDIO_SOURCE: &str = "Audio/Source";
        const AUDIO_SINK: &str = "Audio/Sink";
        const VIDEO_SOURCE: &str = "Video/Source";
        let device_monitor = GstDeviceMonitor::new();
        let audio_caps = Caps::new_simple("audio/x-raw", &[]);
        device_monitor.add_filter(Some(AUDIO_SOURCE), Some(&audio_caps));
        device_monitor.add_filter(Some(AUDIO_SINK), Some(&audio_caps));
        let video_caps = Caps::new_simple("video/x-raw", &[]);
        device_monitor.add_filter(Some(VIDEO_SOURCE), Some(&video_caps));
        let devices = device_monitor
            .get_devices()
            .iter()
            .filter_map(|device| {
                let display_name = device.get_display_name().as_str().to_owned();
                Some(MediaDeviceInfo {
                    device_id: display_name.clone(),
                    kind: match device.get_device_class().as_str() {
                        AUDIO_SOURCE => MediaDeviceKind::AudioInput,
                        AUDIO_SINK => MediaDeviceKind::AudioOutput,
                        VIDEO_SOURCE => MediaDeviceKind::VideoInput,
                        _ => return None,
                    },
                    label: display_name,
                })
            })
            .collect();
        Ok(devices)
    }
}

impl MediaDeviceMonitor for GStreamerDeviceMonitor {
    fn enumerate_devices(&self) -> Result<Vec<MediaDeviceInfo>, ()> {
        {
            if let Some(ref devices) = *self.devices.borrow() {
                return Ok(devices.clone());
            }
        }
        let devices = self.get_devices()?;
        *self.devices.borrow_mut() = Some(devices.clone());
        Ok(devices)
    }
}
