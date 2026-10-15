//! Sound-active trigger: a bass-energy beat detector on the default microphone,
//! standing in for the Obey 40's built-in mic. Built with `--features audio`.

use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::beat::BeatDetector;
use crate::engine::Controller;

pub struct Mic {
    _stream: cpal::Stream,
}

impl Mic {
    pub fn start(controller: Arc<Mutex<Controller>>) -> Result<Self, String> {
        let device = cpal::default_host()
            .default_input_device()
            .ok_or("no input device")?;
        let config = device.default_input_config().map_err(|e| e.to_string())?;
        let rate = config.sample_rate() as f32;
        let channels = config.channels() as usize;
        let mut detector = BeatDetector::new(rate);
        let mut on_samples = move |frames: &mut dyn Iterator<Item = f32>| {
            if detector.process(frames.step_by(channels.max(1)))
                && let Ok(mut c) = controller.lock()
            {
                c.beat();
            }
        };
        let err = |e| eprintln!("audio: {e}");
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream::<f32, _, _>(
                config.into(),
                move |d: &[f32], _: &_| on_samples(&mut d.iter().copied()),
                err,
                None,
            ),
            cpal::SampleFormat::I16 => device.build_input_stream::<i16, _, _>(
                config.into(),
                move |d: &[i16], _: &_| on_samples(&mut d.iter().map(|&s| s as f32 / 32768.0)),
                err,
                None,
            ),
            other => return Err(format!("unsupported sample format {other:?}")),
        }
        .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Self { _stream: stream })
    }
}
