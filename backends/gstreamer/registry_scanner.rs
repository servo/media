use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::str::FromStr;

// The GStreamer registry holds the metadata of the set of plugins available in the host.
// This scanner is used to lazily analyze the registry and to provide information about
// the set of supported mime types and codecs that the backend is able to deal with.
pub static GSTREAMER_REGISTRY_SCANNER: Lazy<GStreamerRegistryScanner> =
    Lazy::new(|| GStreamerRegistryScanner::new());

pub struct GStreamerRegistryScanner {
    supported_mime_types: HashSet<&'static str>,
    supported_codecs: HashSet<&'static str>,
}

impl GStreamerRegistryScanner {
    fn new() -> GStreamerRegistryScanner {
        let mut registry_scanner = GStreamerRegistryScanner {
            supported_mime_types: HashSet::new(),
            supported_codecs: HashSet::new(),
        };
        registry_scanner.initialize();
        registry_scanner
    }

    pub fn is_container_type_supported(&self, container_type: &str) -> bool {
        self.supported_mime_types.contains(container_type)
    }

    fn is_codec_supported(&self, codec: &str) -> bool {
        for supported_codec in &self.supported_codecs {
            if codec.contains(supported_codec) {
                return true;
            }
        }
        false
    }

    pub fn are_all_codecs_supported(&self, codecs: &Vec<&str>) -> bool {
        codecs.iter().all(|&codec| self.is_codec_supported(codec))
    }

    fn initialize(&mut self) {
        let audio_decoder_factories = gst::ElementFactory::factories_with_type(
            gst::ElementFactoryType::DECODER | gst::ElementFactoryType::MEDIA_AUDIO,
            gst::Rank::MARGINAL,
        );
        let audio_parser_factories = gst::ElementFactory::factories_with_type(
            gst::ElementFactoryType::PARSER | gst::ElementFactoryType::MEDIA_AUDIO,
            gst::Rank::NONE,
        );
        let video_decoder_factories = gst::ElementFactory::factories_with_type(
            gst::ElementFactoryType::DECODER | gst::ElementFactoryType::MEDIA_VIDEO,
            gst::Rank::MARGINAL,
        );
        let video_parser_factories = gst::ElementFactory::factories_with_type(
            gst::ElementFactoryType::PARSER | gst::ElementFactoryType::MEDIA_VIDEO,
            gst::Rank::MARGINAL,
        );
        let demux_factories = gst::ElementFactory::factories_with_type(
            gst::ElementFactoryType::DEMUXER,
            gst::Rank::MARGINAL,
        );

        let is_opus_supported = has_element_for_media_type(&audio_parser_factories, "audio/x-opus")
            && has_element_for_media_type(
                &audio_decoder_factories,
                "audio/x-opus, channel-mapping-family=(int)0",
            );
        if is_opus_supported {
            self.supported_mime_types.insert("audio/opus");
            self.supported_codecs.insert("opus");
            self.supported_codecs.insert("x-opus");
        }

        let is_vorbis_supported =
            has_element_for_media_type(&audio_parser_factories, "audio/x-vorbis")
                && has_element_for_media_type(&audio_decoder_factories, "audio/x-vorbis");
        if is_vorbis_supported {
            self.supported_codecs.insert("vorbis");
            self.supported_codecs.insert("x-vorbis");
        }

        // <https://developer.mozilla.org/en-US/docs/Web/Media/Guides/Formats/Containers#webm>
        if has_element_for_media_type(&demux_factories, "audio/webm") {
            if is_opus_supported || is_vorbis_supported {
                self.supported_mime_types.insert("audio/webm");
            }
        }

        let is_vp8_supported = has_element_for_media_type(&video_decoder_factories, "video/x-vp8");
        if is_vp8_supported {
            self.supported_codecs.insert("vp8");
            self.supported_codecs.insert("x-vp8");
            self.supported_codecs.insert("vp8.0");
            self.supported_codecs.insert("vp08");
        }

        let is_vp9_supported = has_element_for_media_type(&video_parser_factories, "video/x-vp9")
            && has_element_for_media_type(&video_decoder_factories, "video/x-vp9");
        if is_vp9_supported {
            self.supported_codecs.insert("vp9");
            self.supported_codecs.insert("x-vp9");
            self.supported_codecs.insert("vp9.0");
            self.supported_codecs.insert("vp09");
        }

        let is_av1_supported = has_element_for_media_type(&video_parser_factories, "video/x-av1")
            && has_element_for_media_type(&video_decoder_factories, "video/x-av1");
        if is_av1_supported {
            self.supported_codecs.insert("av1");
            self.supported_codecs.insert("x-av1");
            self.supported_codecs.insert("av01");
        }

        // <https://developer.mozilla.org/en-US/docs/Web/Media/Guides/Formats/Containers#webm>
        if has_element_for_media_type(&demux_factories, "video/webm") {
            if is_vp8_supported || is_vp9_supported || is_av1_supported {
                self.supported_mime_types.insert("video/webm");
            }
        }

        let is_aac_supported =
            has_element_for_media_type(&audio_parser_factories, "audio/mpeg, mpegversion=(int)4")
                && has_element_for_media_type(
                    &audio_decoder_factories,
                    "audio/mpeg, mpegversion=(int)4, stream-format=(string)adts",
                );
        if is_aac_supported {
            self.supported_mime_types.insert("audio/aac");
            self.supported_codecs.insert("mpeg");
            self.supported_codecs.insert("mp4a");
        }

        let is_mpeg4v_supported = has_element_for_media_type(
            &video_parser_factories,
            "video/mpeg, mpegversion=(int)4, systemstream=(boolean)false",
        ) && has_element_for_media_type(
            &video_decoder_factories,
            "video/mpeg, mpegversion=(int)4, systemstream=(boolean)false",
        );
        if is_mpeg4v_supported {
            self.supported_codecs.insert("mp4v");
        }

        let mut is_h264_supported = false;
        if has_element_for_media_type(&video_parser_factories, "video/x-h264") {
            let is_h264_avc1_supported = has_element_for_media_type(
                &video_decoder_factories,
                "video/x-h264, stream-format=(string)avc, alignment=(string)au",
            );

            let is_h264_avc3_supported = has_element_for_media_type(
                &video_decoder_factories,
                "video/x-h264, stream-format=(string)avc3, alignment=(string)au",
            );

            if is_h264_avc1_supported {
                self.supported_codecs.insert("avc1");
            }

            if is_h264_avc3_supported {
                self.supported_codecs.insert("avc3");
            }

            if is_h264_avc1_supported || is_h264_avc3_supported {
                self.supported_codecs.insert("x-h264");
                is_h264_supported = true;
            }
        };

        let mut is_h265_supported = false;
        if has_element_for_media_type(&video_parser_factories, "video/x-h265") {
            let is_h265_hvc1_supported = has_element_for_media_type(
                &video_decoder_factories,
                "video/x-h265, stream-format=(string)hvc1, alignment=(string)au",
            );

            let is_h265_hev1_supported = has_element_for_media_type(
                &video_decoder_factories,
                "video/x-h265, stream-format=(string)hev1, alignment=(string)au",
            );

            if is_h265_hvc1_supported {
                self.supported_codecs.insert("hvc1");
            }

            if is_h265_hev1_supported {
                self.supported_codecs.insert("hev1");
            }

            if is_h265_hvc1_supported || is_h265_hev1_supported {
                self.supported_codecs.insert("x-h265");
                is_h265_supported = true;
            }
        };

        // <https://developer.mozilla.org/en-US/docs/Web/Media/Guides/Formats/Containers#mpeg-4_mp4>
        if has_element_for_media_type(&demux_factories, "video/quicktime") {
            if is_aac_supported || is_opus_supported {
                self.supported_mime_types.insert("audio/mp4");
                self.supported_mime_types.insert("audio/x-m4a");
            }

            if is_mpeg4v_supported
                || is_h264_supported
                || is_h265_supported
                || is_av1_supported
                || is_vp9_supported
            {
                self.supported_mime_types.insert("video/mp4");
            }
        }

        if has_element_for_media_type(&audio_decoder_factories, "audio/midi") {
            self.supported_mime_types.insert("audio/midi");
            self.supported_mime_types.insert("audio/riff-midi");
        }

        if has_element_for_media_type(&audio_decoder_factories, "audio/x-ac3") {
            self.supported_mime_types.insert("audio/x-ac3");
        }

        if has_element_for_media_type(&audio_decoder_factories, "audio/x-flac") {
            self.supported_mime_types.insert("audio/flac");
            self.supported_mime_types.insert("audio/x-flac");
        }

        if has_element_for_media_type(&audio_decoder_factories, "audio/x-speex") {
            self.supported_mime_types.insert("audio/speex");
            self.supported_mime_types.insert("audio/x-speex");
        }

        if has_element_for_media_type(&audio_decoder_factories, "audio/x-wavpack") {
            self.supported_mime_types.insert("audio/x-wavpack");
        }

        if has_element_for_media_type(
            &video_decoder_factories,
            "video/mpeg, mpegversion=(int){1,2}, systemstream=(boolean)false",
        ) {
            self.supported_mime_types.insert("video/mpeg");
            self.supported_codecs.insert("mpeg");
        }

        if has_element_for_media_type(&video_decoder_factories, "video/x-flash-video") {
            self.supported_mime_types.insert("video/flv");
            self.supported_mime_types.insert("video/x-flv");
        }

        if has_element_for_media_type(&video_decoder_factories, "video/x-msvideocodec") {
            self.supported_mime_types.insert("video/x-msvideo");
        }

        if has_element_for_media_type(&demux_factories, "application/x-hls") {
            self.supported_mime_types
                .insert("application/vnd.apple.mpegurl");
            self.supported_mime_types.insert("application/x-mpegurl");
        }

        if has_element_for_media_type(&demux_factories, "application/x-wav")
            || has_element_for_media_type(&demux_factories, "audio/x-wav")
        {
            self.supported_mime_types.insert("audio/wav");
            self.supported_mime_types.insert("audio/vnd.wav");
            self.supported_mime_types.insert("audio/x-wav");
            self.supported_codecs.insert("1");
        }

        if has_element_for_media_type(&demux_factories, "application/ogg") {
            self.supported_mime_types.insert("application/ogg");

            if is_vorbis_supported {
                self.supported_mime_types.insert("audio/ogg");
                self.supported_mime_types.insert("audio/x-vorbis+ogg");
            }

            if has_element_for_media_type(&audio_decoder_factories, "audio/x-speex") {
                self.supported_mime_types.insert("audio/ogg");
                self.supported_codecs.insert("speex");
            }

            if has_element_for_media_type(&video_decoder_factories, "video/x-theora") {
                self.supported_mime_types.insert("video/ogg");
                self.supported_codecs.insert("theora");
            }
        }

        let mut is_audio_mpeg_supported = false;
        if has_element_for_media_type(
            &audio_decoder_factories,
            "audio/mpeg, mpegversion=(int)1, layer=(int)[1, 3]",
        ) {
            is_audio_mpeg_supported = true;
            self.supported_mime_types.insert("audio/mp1");
            self.supported_mime_types.insert("audio/mp3");
            self.supported_mime_types.insert("audio/x-mp3");
            self.supported_codecs.insert("audio/mp3");
        }

        if has_element_for_media_type(&audio_decoder_factories, "audio/mpeg, mpegversion=(int)2") {
            is_audio_mpeg_supported = true;
            self.supported_mime_types.insert("audio/mp2");
        }

        is_audio_mpeg_supported |= self.is_container_type_supported("video/mp4");
        if is_audio_mpeg_supported {
            self.supported_mime_types.insert("audio/mpeg");
            self.supported_mime_types.insert("audio/x-mpeg");
        }

        if has_element_for_media_type(&demux_factories, "video/x-matroska") {
            self.supported_mime_types.insert("video/x-matroska");
        }
    }
}

fn has_element_for_media_type(
    factories: &glib::List<gst::ElementFactory>,
    media_type: &str,
) -> bool {
    match gst::caps::Caps::from_str(media_type) {
        Ok(caps) => {
            for factory in factories {
                if factory.can_sink_all_caps(&caps) {
                    return true;
                }
            }
            false
        },
        _ => false,
    }
}
