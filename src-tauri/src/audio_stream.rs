use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use anyhow::{anyhow, Context, Result};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Sample, SampleFormat, SizedSample, Stream,
};
use tokio::sync::mpsc;

/// Continuous audio capture for ElevenLabs streaming
pub struct ContinuousAudioCapture {
    stream: Option<Stream>,
    is_running: Arc<AtomicBool>,
    audio_tx: Option<mpsc::UnboundedSender<Vec<u8>>>,
    sample_rate: u32,
    channels: u16,
}

impl ContinuousAudioCapture {
    pub fn new() -> Result<Self> {
        Ok(Self {
            stream: None,
            is_running: Arc::new(AtomicBool::new(false)),
            audio_tx: None,
            sample_rate: 0,
            channels: 0,
        })
    }

    /// Starts continuous audio capture
    /// Returns a receiver for audio chunks (PCM16 little-endian)
    pub fn start(&mut self) -> Result<mpsc::UnboundedReceiver<Vec<u8>>> {
        if self.is_running.load(Ordering::Relaxed) {
            return Err(anyhow!("Audio capture already running"));
        }

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow!("No input microphone detected"))?;

        let config = device
            .default_input_config()
            .context("Failed to query default input configuration")?;

        let sample_format = config.sample_format();
        let config: cpal::StreamConfig = config.into();

        self.sample_rate = config.sample_rate.0;
        self.channels = config.channels;

        log::info!(
            "[AudioStream] Starting continuous capture: {} Hz, {} channels, format: {:?}",
            self.sample_rate,
            self.channels,
            sample_format
        );

        let (tx, rx) = mpsc::unbounded_channel();
        self.audio_tx = Some(tx.clone());

        let channels = config.channels as usize;
        let chunk_size_ms = 100; // 100ms chunks
        let samples_per_chunk = (self.sample_rate as usize * chunk_size_ms / 1000) * channels;

        let stream = build_streaming_input(
            &device,
            &config,
            sample_format,
            tx,
            channels,
            samples_per_chunk,
        )?;

        stream.play().context("Failed to start audio stream")?;
        self.stream = Some(stream);
        self.is_running.store(true, Ordering::Relaxed);

        log::info!("[AudioStream] Continuous capture started");
        Ok(rx)
    }

    /// Stops continuous audio capture
    pub fn stop(&mut self) -> Result<()> {
        if !self.is_running.load(Ordering::Relaxed) {
            return Ok(());
        }

        log::info!("[AudioStream] Stopping continuous capture");

        if let Some(stream) = self.stream.take() {
            drop(stream);
        }

        self.audio_tx = None;
        self.is_running.store(false, Ordering::Relaxed);

        log::info!("[AudioStream] Continuous capture stopped");
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }
}

impl Drop for ContinuousAudioCapture {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Builds CPAL input stream that sends PCM16 chunks via channel
fn build_streaming_input(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sample_format: SampleFormat,
    tx: mpsc::UnboundedSender<Vec<u8>>,
    channels: usize,
    chunk_size: usize,
) -> Result<Stream> {
    let err_fn = |err| {
        log::error!("[AudioStream] Stream error: {}", err);
    };

    match sample_format {
        SampleFormat::F32 => build_stream::<f32>(device, config, tx, err_fn, channels, chunk_size, convert_f32_to_i16),
        SampleFormat::F64 => build_stream::<f64>(device, config, tx, err_fn, channels, chunk_size, |s| convert_f32_to_i16(s as f32)),
        SampleFormat::I16 => build_stream::<i16>(device, config, tx, err_fn, channels, chunk_size, |s| s),
        SampleFormat::I32 => build_stream::<i32>(device, config, tx, err_fn, channels, chunk_size, |s| (s >> 16) as i16),
        SampleFormat::I8 => build_stream::<i8>(device, config, tx, err_fn, channels, chunk_size, |s| (s as i16) << 8),
        SampleFormat::U16 => build_stream::<u16>(device, config, tx, err_fn, channels, chunk_size, |s| (s as i32 - 32768) as i16),
        other => Err(anyhow!("Unsupported sample format: {:?}", other)),
    }
}

fn build_stream<T: Sample + SizedSample + Send + 'static>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    tx: mpsc::UnboundedSender<Vec<u8>>,
    err_fn: impl Fn(cpal::StreamError) + Send + 'static,
    channels: usize,
    chunk_size: usize,
    convert: fn(T) -> i16,
) -> Result<Stream> {
    // Buffer for accumulating samples until we have a full chunk
    let mut buffer = Vec::with_capacity(chunk_size);

    let stream = device.build_input_stream(
        config,
        move |data: &[T], _| {
            // Convert samples to PCM16 and add to buffer
            for frame in data.chunks(channels) {
                for &sample in frame {
                    let pcm16 = convert(sample);
                    buffer.extend_from_slice(&pcm16.to_le_bytes());
                }
            }

            // If we have enough data, send a chunk
            while buffer.len() >= chunk_size * 2 { // *2 because i16 = 2 bytes
                let chunk: Vec<u8> = buffer.drain(..chunk_size * 2).collect();

                // Send chunk (non-blocking)
                if tx.send(chunk).is_err() {
                    log::warn!("[AudioStream] Receiver dropped, stopping stream");
                    return;
                }
            }
        },
        err_fn,
        None,
    )?;

    Ok(stream)
}

/// Converts f32 sample to i16 PCM
fn convert_f32_to_i16(sample: f32) -> i16 {
    let clamped = sample.clamp(-1.0, 1.0);
    (clamped * i16::MAX as f32) as i16
}
