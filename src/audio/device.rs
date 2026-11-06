//! Mac and Windows output: the `cpal` crate, straight to CoreAudio or
//! WASAPI. The device picks the sample rate, format and channel count; the
//! mixer renders mono at that rate and the same sample goes to every channel.

use std::sync::mpsc::Receiver;
use std::thread;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, FromSample, SampleFormat, SizedSample, Stream, StreamConfig};

use super::{Mixer, Request};

pub fn run(rx: Receiver<Request>) {
    let Some(stream) = open(rx) else {
        return;
    };
    if stream.play().is_err() {
        return;
    }
    // The stream plays for as long as it lives (and must stay on the
    // thread that made it), so keep this thread until the game exits.
    loop {
        thread::park();
    }
}

/// The default output device, in whatever sample format it prefers.
fn open(rx: Receiver<Request>) -> Option<Stream> {
    let device = cpal::default_host().default_output_device()?;
    let supported = device.default_output_config().ok()?;
    let format = supported.sample_format();
    let config: StreamConfig = supported.into();
    match format {
        SampleFormat::F32 => build::<f32>(&device, &config, rx),
        SampleFormat::I16 => build::<i16>(&device, &config, rx),
        SampleFormat::U16 => build::<u16>(&device, &config, rx),
        _ => None,
    }
}

fn build<T>(device: &Device, config: &StreamConfig, rx: Receiver<Request>) -> Option<Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels.max(1) as usize;
    let mut mixer = Mixer::new(config.sample_rate.0);
    let mut mono: Vec<f32> = Vec::new();
    let fill = move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
        // New requests start on the next buffer, a few milliseconds away.
        mixer.take_requests(&rx);
        mono.resize(data.len() / channels, 0.0);
        mixer.render_f32(&mut mono);
        for (frame, &s) in data.chunks_mut(channels).zip(&mono) {
            let v = T::from_sample(s);
            frame.iter_mut().for_each(|x| *x = v);
        }
    };
    device.build_output_stream(config, fill, |_| {}, None).ok()
}
