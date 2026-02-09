use std::{
    io::Cursor,
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Instant,
};

use anyhow::{anyhow, Context, Result};
use cpal::{
    traits::DeviceTrait, traits::HostTrait, traits::StreamTrait, Sample, SampleFormat, SizedSample,
    Stream,
};
use hound::{SampleFormat as WavSampleFormat, WavSpec, WavWriter};

pub struct Recorder;

pub struct RecordingSession {
    stop_tx: Option<mpsc::Sender<()>>,
    handle: Option<thread::JoinHandle<Result<RecordingResult>>>,
    started_at: Instant,
}

struct RecordingResult {
    buffer: Vec<f32>,
    sample_rate: u32,
    channels: u16,
}

impl Recorder {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    pub fn start(&self) -> Result<RecordingSession> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow!("No input microphone detected"))?;
        let config = device
            .default_input_config()
            .context("Failed to query default input configuration")?;
        let sample_format = config.sample_format();
        let config: cpal::StreamConfig = config.into();
        let (stop_tx, stop_rx) = mpsc::channel();

        let handle = thread::spawn(move || -> Result<RecordingResult> {
            let channels = config.channels as usize;
            let sample_rate = config.sample_rate.0;
            let max_samples = (sample_rate as usize) * channels * 120; // 2 minutes max
            let buffer = Arc::new(Mutex::new(Vec::<f32>::with_capacity(max_samples)));
            let buffer_clone = buffer.clone();
            let buffer_for_err = buffer.clone();

            let err_fn = move |err| {
                eprintln!("Audio stream error: {err}");
                if let Ok(mut buf) = buffer_for_err.lock() {
                    buf.push(f32::NAN);
                }
            };

            let stream = build_stream(&device, &config, sample_format, buffer_clone, err_fn)?;
            stream.play().context("Failed to start audio stream")?;

            let _ = stop_rx.recv();
            drop(stream);

            let mut data = buffer
                .lock()
                .map_err(|_| anyhow!("Failed to access recorded samples"))?;
            let collected = std::mem::take(&mut *data);
            Ok(RecordingResult {
                buffer: collected,
                sample_rate,
                channels: config.channels,
            })
        });

        Ok(RecordingSession {
            stop_tx: Some(stop_tx),
            handle: Some(handle),
            started_at: Instant::now(),
        })
    }
}

impl Drop for RecordingSession {
    fn drop(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl RecordingSession {
    pub fn stop(mut self) -> Result<Vec<u8>> {
        if self.started_at.elapsed().as_millis() < 120 {
            return Err(anyhow!(
                "Recording stopped too quickly. Hold the hotkey for a little longer before releasing."
            ));
        }

        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        let handle = self
            .handle
            .take()
            .ok_or_else(|| anyhow!("Recording session already finished"))?;

        let result = handle
            .join()
            .map_err(|_| anyhow!("Failed to join recording thread"))??;

        if result.buffer.is_empty() {
            return Err(anyhow!("Recording produced no audio"));
        }

        let mut cursor = Cursor::new(Vec::with_capacity(result.buffer.len() * 2));
        let mut writer = WavWriter::new(
            &mut cursor,
            WavSpec {
                channels: result.channels,
                sample_rate: result.sample_rate,
                bits_per_sample: 16,
                sample_format: WavSampleFormat::Int,
            },
        )
        .context("Failed to create WAV writer")?;

        for sample in result.buffer {
            let amp = (sample * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
            writer
                .write_sample(amp)
                .context("Failed to write WAV sample")?;
        }

        writer.finalize().context("Failed to finalize WAV output")?;
        Ok(cursor.into_inner())
    }
}

fn build_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sample_format: SampleFormat,
    buffer: Arc<Mutex<Vec<f32>>>,
    err_fn: impl Fn(cpal::StreamError) + Send + 'static,
) -> Result<Stream> {
    let channels = config.channels as usize;
    let max_samples = config.sample_rate.0 as usize * channels * 120;

    match sample_format {
        SampleFormat::F32 => {
            build::<f32>(device, config, buffer, err_fn, channels, max_samples, |s| s)
        }
        SampleFormat::F64 => {
            build::<f64>(device, config, buffer, err_fn, channels, max_samples, |s| {
                s as f32
            })
        }
        SampleFormat::I16 => {
            build::<i16>(device, config, buffer, err_fn, channels, max_samples, |s| {
                s as f32 / i16::MAX as f32
            })
        }
        SampleFormat::I32 => {
            build::<i32>(device, config, buffer, err_fn, channels, max_samples, |s| {
                (s as f64 / i32::MAX as f64) as f32
            })
        }
        SampleFormat::I8 => {
            build::<i8>(device, config, buffer, err_fn, channels, max_samples, |s| {
                s as f32 / i8::MAX as f32
            })
        }
        SampleFormat::I64 => {
            build::<i64>(device, config, buffer, err_fn, channels, max_samples, |s| {
                (s as f64 / i64::MAX as f64) as f32
            })
        }
        SampleFormat::U8 => {
            build::<u8>(device, config, buffer, err_fn, channels, max_samples, |s| {
                (s as f32 / u8::MAX as f32) * 2.0 - 1.0
            })
        }
        SampleFormat::U16 => {
            build::<u16>(device, config, buffer, err_fn, channels, max_samples, |s| {
                (s as f32 / u16::MAX as f32) * 2.0 - 1.0
            })
        }
        SampleFormat::U32 => {
            build::<u32>(device, config, buffer, err_fn, channels, max_samples, |s| {
                ((s as f64) / u32::MAX as f64 * 2.0 - 1.0) as f32
            })
        }
        SampleFormat::U64 => {
            build::<u64>(device, config, buffer, err_fn, channels, max_samples, |s| {
                ((s as f64) / u64::MAX as f64 * 2.0 - 1.0) as f32
            })
        }
        other => Err(anyhow!("Unsupported sample format: {other:?}")),
    }
}

fn build<T: Sample + SizedSample + 'static>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    buffer: Arc<Mutex<Vec<f32>>>,
    err_fn: impl Fn(cpal::StreamError) + Send + 'static,
    channels: usize,
    max_samples: usize,
    convert: fn(T) -> f32,
) -> Result<Stream> {
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _| {
            if let Ok(mut buf) = buffer.lock() {
                if buf.len() >= max_samples {
                    return;
                }
                buf.reserve(data.len());
                for frame in data.chunks(channels) {
                    for &sample in frame {
                        buf.push(convert(sample));
                    }
                }
            }
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}
