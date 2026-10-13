use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use soc_core::Result;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender},
};

pub struct Capture {
    stream: Option<cpal::Stream>,
    pub frames: Receiver<Vec<f32>>,
    failed: Arc<AtomicBool>,
    tail: Arc<Mutex<Vec<f32>>>,
}
pub fn input_devices() -> Result<Vec<String>> {
    let mut names = cpal::default_host()
        .input_devices()?
        .map(|device| device.name().map_err(Into::into))
        .collect::<Result<Vec<_>>>()?;
    names.sort();
    names.dedup();
    Ok(names)
}
impl Capture {
    pub fn start(name: Option<&str>) -> Result<Self> {
        let host = cpal::default_host();
        let device = if let Some(name) = name {
            let mut matches = host
                .input_devices()?
                .filter(|device| device.name().is_ok_and(|value| value == name));
            let device = matches
                .next()
                .ok_or("El micrófono elegido no está conectado. Selecciona otra entrada")?;
            if matches.next().is_some() {
                return Err("Hay varios micrófonos con ese nombre. Renómbralos o usa la entrada del sistema".into());
            }
            device
        } else {
            host.default_input_device()
                .ok_or("No hay micrófono disponible")?
        };
        let supported = device.default_input_config()?;
        let channels = supported.channels() as usize;
        let rate = supported.sample_rate().0;
        let config = supported.config();
        let (tx, frames) = mpsc::sync_channel(512);
        let failed = Arc::new(AtomicBool::new(false));
        let tail = Arc::new(Mutex::new(Vec::with_capacity(1600)));
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => build::<f32>(
                &device,
                &config,
                channels,
                rate,
                tx,
                failed.clone(),
                tail.clone(),
            )?,
            cpal::SampleFormat::I16 => build::<i16>(
                &device,
                &config,
                channels,
                rate,
                tx,
                failed.clone(),
                tail.clone(),
            )?,
            cpal::SampleFormat::U16 => build::<u16>(
                &device,
                &config,
                channels,
                rate,
                tx,
                failed.clone(),
                tail.clone(),
            )?,
            _ => return Err("Formato del micrófono no compatible".into()),
        };
        stream.play()?;
        Ok(Self {
            stream: Some(stream),
            frames,
            failed,
            tail,
        })
    }
    pub fn check(&self) -> Result<()> {
        if self.failed.load(Ordering::Relaxed) {
            Err("El micrófono falló o la transcripción no alcanzó a procesar el audio".into())
        } else {
            Ok(())
        }
    }
    pub fn stop(&mut self) -> Vec<f32> {
        self.stream.take();
        self.tail
            .lock()
            .map(|mut tail| std::mem::take(&mut *tail))
            .unwrap_or_default()
    }
}
fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    rate: u32,
    tx: SyncSender<Vec<f32>>,
    failed: Arc<AtomicBool>,
    tail: Arc<Mutex<Vec<f32>>>,
) -> Result<cpal::Stream>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let mut resampler = Resampler::new(rate);
    let mut input = MonoInput::new(channels);
    let error_flag = failed.clone();
    Ok(device.build_input_stream(
        config,
        move |samples: &[T], _| {
            let Ok(mut frame) = tail.lock() else {
                failed.store(true, Ordering::Relaxed);
                return;
            };
            input.convert(samples, |mono| {
                resampler.push(mono, |value| {
                    frame.push(value);
                    if frame.len() == 1600 {
                        let complete = std::mem::replace(&mut *frame, Vec::with_capacity(1600));
                        if tx.try_send(complete).is_err() {
                            failed.store(true, Ordering::Relaxed);
                        }
                    }
                });
            });
        },
        move |_| {
            error_flag.store(true, Ordering::Relaxed);
        },
        None,
    )?)
}
// Conserva un canal de entrada: promediar voz con canales vacíos atenúa la señal.
// Histéresis de 6 dB entre bloques evita alternar canales por ruido semejante.
struct MonoInput {
    selected: usize,
    energy: Vec<f64>,
}
impl MonoInput {
    fn new(channels: usize) -> Self {
        Self {
            selected: 0,
            energy: vec![0.0; channels],
        }
    }
    fn convert<T>(&mut self, samples: &[T], mut emit: impl FnMut(f32))
    where
        T: cpal::Sample,
        f32: cpal::FromSample<T>,
    {
        self.energy.fill(0.0);
        for frame in samples.chunks_exact(self.energy.len()) {
            for (channel, sample) in frame.iter().enumerate() {
                let value = sample.to_sample::<f32>() as f64;
                self.energy[channel] += value * value;
            }
        }
        if let Some((best, energy)) = self
            .energy
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            && *energy > self.energy[self.selected] * 4.0
        {
            self.selected = best;
        }
        for frame in samples.chunks_exact(self.energy.len()) {
            emit(frame[self.selected].to_sample::<f32>());
        }
    }
}

// Interpolación continua entre callbacks; la fase no se reinicia por bloque.
struct Resampler {
    previous: Option<f32>,
    position: u64,
    next: f64,
    step: f64,
}
impl Resampler {
    fn new(rate: u32) -> Self {
        Self {
            previous: None,
            position: 0,
            next: 0.0,
            step: rate as f64 / 16000.0,
        }
    }
    fn push(&mut self, value: f32, mut emit: impl FnMut(f32)) {
        if let Some(previous) = self.previous {
            while self.next <= self.position as f64 {
                let fraction = (self.next - (self.position - 1) as f64) as f32;
                emit(previous + (value - previous) * fraction.clamp(0.0, 1.0));
                self.next += self.step;
            }
        } else {
            emit(value);
            self.next += self.step;
        }
        self.previous = Some(value);
        self.position += 1;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mono_input_preserves_voice_with_an_empty_or_opposite_channel() {
        for stereo in [
            vec![0.25, 0.0, -0.25, 0.0],
            vec![0.0, 0.25, 0.0, -0.25],
            vec![0.25, -0.25, -0.25, 0.25],
        ] {
            let mut input = MonoInput::new(2);
            let mut mono = Vec::new();
            input.convert(&stereo, |value| mono.push(value));
            assert_eq!(mono, vec![0.25, -0.25]);
        }
        let mut input = MonoInput::new(1);
        let mut mono = Vec::new();
        input.convert(&[0.1_f32, -0.1], |value| mono.push(value));
        assert_eq!(mono, vec![0.1, -0.1]);
    }
    #[test]
    fn mono_input_keeps_channel_until_another_is_clearly_stronger() {
        let mut input = MonoInput::new(2);
        input.convert(&[0.0_f32, 0.2], |_| {});
        assert_eq!(input.selected, 1);
        input.convert(&[0.22_f32, 0.2], |_| {});
        assert_eq!(input.selected, 1);
        input.convert(&[0.5_f32, 0.1], |_| {});
        assert_eq!(input.selected, 0);
    }
    #[test]
    fn resampler_preserves_phase_and_duration() {
        for rate in [16000, 44100, 48000] {
            let mut r = Resampler::new(rate);
            let mut result = Vec::new();
            for _ in 0..rate {
                r.push(0.5, |v| result.push(v));
            }
            assert!((result.len() as i32 - 16000).abs() <= 1);
            assert!(result.iter().all(|v| (*v - 0.5).abs() < 0.0001));
        }
    }
}
