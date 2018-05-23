use std::sync::mpsc::{Sender, Receiver, channel};
use audio::block::Chunk;
use audio::graph_thread::AudioGraphThreadMsg;
use super::gst_app::{AppSrc, AppSrcCallbacks};
use super::gst_audio;
use audio::block::FRAMES_PER_BLOCK;
use audio::sink::AudioSink;
use gst;
use gst::prelude::*;

// XXX Define own error type.

pub struct GStreamerAudioSink {
    pipeline: gst::Pipeline,
    chunk_sender: Sender<Chunk>,
    chunk_receiver: Option<Receiver<Chunk>>,
    graph: Sender<AudioGraphThreadMsg>
}

impl GStreamerAudioSink {
    pub fn new(graph: Sender<AudioGraphThreadMsg>) -> Self {
        let (sender, receiver) = channel();
        Self {
            pipeline: gst::Pipeline::new(None),
            chunk_sender: sender,
            chunk_receiver: Some(receiver),
            graph
        }
    }
}

impl AudioSink for GStreamerAudioSink {
    fn init(&mut self, rate: u32) -> Result<(), ()> {
        if let Some(category) = gst::DebugCategory::get("openslessink") {
            category.set_threshold(gst::DebugLevel::Trace);
        }
        gst::init().map_err(|_| ())?;

        let src = gst::ElementFactory::make("appsrc", None).ok_or(())?;
        let src = src.downcast::<AppSrc>().map_err(|_| ())?;
        let info = gst_audio::AudioInfo::new(gst_audio::AUDIO_FORMAT_F32, rate, 1)
            .build()
            .ok_or(())?;
        src.set_caps(&info.to_caps().unwrap());
        src.set_property_format(gst::Format::Time);
        let mut sample_offset = 0;
        let n_samples = FRAMES_PER_BLOCK as u64;
        let buf_size = (n_samples as usize) * (info.bpf() as usize);

        assert!(info.bpf() == 4);
        let rate = info.rate();

        let chunk_receiver = self.chunk_receiver.take().expect("Already initialized");

        let graph = self.graph.clone();
        let need_data = move |app: &AppSrc, _bytes: u32| {

            let mut process_chunk = |mut chunk: Chunk| {
                let mut buffer = gst::Buffer::with_size(buf_size).unwrap();
                {
                    let buffer = buffer.get_mut().unwrap();
                    // Calculate the current timestamp (PTS) and the next one,
                    // and calculate the duration from the difference instead of
                    // simply the number of samples to prevent rounding errors
                    let pts = sample_offset
                        .mul_div_floor(gst::SECOND_VAL, rate as u64)
                        .unwrap()
                        .into();
                    let next_pts: gst::ClockTime = (sample_offset + n_samples)
                        .mul_div_floor(gst::SECOND_VAL, rate as u64)
                        .unwrap()
                        .into();
                    buffer.set_pts(pts);
                    buffer.set_duration(next_pts - pts);

                    
                    // let mut chunks = graph_.process(rate);
                    // sometimes nothing reaches the output
                    if chunk.len() == 0 {
                        chunk.blocks.push(Default::default());
                        info.format_info().fill_silence(chunk.blocks[0].as_mut_byte_slice());
                    }
                    debug_assert!(chunk.len() == 1);
                    let data = chunk.blocks[0].as_mut_byte_slice();

                    // XXXManishearth if we have a safe way to convert
                    // from Box<[f32]> to Box<[u8]> (similarly for Vec)
                    // we can use Buffer::from_slice instead
                    buffer.copy_from_slice(0, data).expect("copying failed");

                    sample_offset += n_samples;
                }
                let _ = app.push_buffer(buffer);
            };


            let mut processed = false;

            // we may have extra chunks, might as well process them
            while let Ok(chunk) = chunk_receiver.try_recv() {
                processed = true;
                process_chunk(chunk);
            }

            if processed {
                return;
            }

            graph.send(AudioGraphThreadMsg::SinkNeedsData).unwrap();
            // block till we get the first chunk
            process_chunk(chunk_receiver.recv().unwrap());


        };
        src.set_callbacks(AppSrcCallbacks::new().need_data(need_data).build());

        let src = src.upcast();
        let resample = gst::ElementFactory::make("audioresample", None).ok_or(())?;
        let convert = gst::ElementFactory::make("audioconvert", None).ok_or(())?;
        let sink = gst::ElementFactory::make("autoaudiosink", None).ok_or(())?;
        self.pipeline
            .add_many(&[&src, &resample, &convert, &sink])
            .map_err(|_| ())?;
        gst::Element::link_many(&[&src, &resample, &convert, &sink]).map_err(|_| ())?;

        Ok(())
    }

    fn send_chunk(&self, chunk: Chunk) {
        self.chunk_sender.send(chunk).unwrap();
    }

    fn play(&self) {
        let _ = self.pipeline.set_state(gst::State::Playing);
    }

    fn stop(&self) {
        let _ = self.pipeline.set_state(gst::State::Paused);
    }
}

impl Drop for GStreamerAudioSink {
    fn drop(&mut self) {
        self.stop();
    }
}
