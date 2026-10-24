//! Sonda de niveles de entrada: no guarda audio ni ejecuta ASR.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use gennie_core::Result;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Levels {
    frames: usize,
    energy: Vec<f64>,
    peak: Vec<f32>,
    mono_energy: f64,
    mono_peak: f32,
    clipped: usize,
    first_frame_ms: Option<u128>,
}
fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    levels: Arc<Mutex<Levels>>,
    failed: Arc<AtomicBool>,
    started: Instant,
) -> Result<cpal::Stream>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let channels = config.channels as usize;
    let callback_failed = failed.clone();
    Ok(device.build_input_stream(
        config,
        move |samples: &[T], _| {
            let Ok(mut levels) = levels.lock() else {
                callback_failed.store(true, Ordering::Relaxed);
                return;
            };
            for frame in samples.chunks_exact(channels) {
                levels
                    .first_frame_ms
                    .get_or_insert_with(|| started.elapsed().as_millis());
                let mut mono = 0.0;
                for (channel, sample) in frame.iter().enumerate() {
                    let value = sample.to_sample::<f32>();
                    levels.energy[channel] += (value as f64).powi(2);
                    levels.peak[channel] = levels.peak[channel].max(value.abs());
                    if value.abs() >= 0.999 {
                        levels.clipped += 1;
                    }
                    mono += value / channels as f32;
                }
                levels.mono_energy += (mono as f64).powi(2);
                levels.mono_peak = levels.mono_peak.max(mono.abs());
                levels.frames += 1;
            }
        },
        move |_| {
            failed.store(true, Ordering::Relaxed);
        },
        None,
    )?)
}
fn db(value: f64) -> f64 {
    20.0 * value.max(1e-9).log10()
}
fn main() -> Result<()> {
    let name = std::env::args()
        .nth(1)
        .ok_or("Falta nombre exacto de entrada")?;
    let mut devices = cpal::default_host().input_devices()?;
    let device = devices
        .find(|device| device.name().is_ok_and(|value| value == name))
        .ok_or("Entrada no encontrada")?;
    if devices.any(|device| device.name().is_ok_and(|value| value == name)) {
        return Err("Nombre de entrada ambiguo".into());
    }
    let supported = device.default_input_config()?;
    let config = supported.config();
    let channels = config.channels as usize;
    let levels = Arc::new(Mutex::new(Levels {
        energy: vec![0.0; channels],
        peak: vec![0.0; channels],
        ..Default::default()
    }));
    let failed = Arc::new(AtomicBool::new(false));
    let started = Instant::now();
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => {
            build::<f32>(&device, &config, levels.clone(), failed.clone(), started)?
        }
        cpal::SampleFormat::I16 => {
            build::<i16>(&device, &config, levels.clone(), failed.clone(), started)?
        }
        cpal::SampleFormat::U16 => {
            build::<u16>(&device, &config, levels.clone(), failed.clone(), started)?
        }
        _ => return Err("Formato no compatible".into()),
    };
    stream.play()?;
    println!(
        "Habla durante 10 segundos. Entrada: {name}; canales={channels}; rate={}; formato={:?}",
        config.sample_rate.0,
        supported.sample_format()
    );
    for second in 1..=10 {
        std::thread::sleep(Duration::from_secs(1));
        if failed.load(Ordering::Relaxed) {
            return Err("Error de entrada".into());
        }
        let levels = levels.lock().map_err(|_| "Medición inválida")?;
        let count = levels.frames.max(1) as f64;
        let rms: Vec<f64> = levels
            .energy
            .iter()
            .map(|energy| db((energy / count).sqrt()))
            .collect();
        println!(
            "second={second} frames={} first_ms={:?} rms_channels_db={rms:.1?} average_rms_db={:.1} average_peak_db={:.1} clipped={}",
            levels.frames,
            levels.first_frame_ms,
            db((levels.mono_energy / count).sqrt()),
            db(levels.mono_peak as f64),
            levels.clipped
        );
    }
    drop(stream);
    Ok(())
}
