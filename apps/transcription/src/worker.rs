use crate::{audio::Capture, model};
use serde::{Deserialize, Serialize};
use soc_core::Result;
use std::{
    io::{self, BufRead, Write},
    path::Path,
    sync::mpsc,
    time::{Duration, Instant},
};
use transcribe_cpp::{
    Model, ParakeetStreamOptions, RunOptions, SessionOptions, StreamExtension, StreamOptions,
    TimestampKind,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Event {
    Status(String),
    Download(u8),
    Ready,
    ModelLoaded,
    DictationStarted,
    Listening,
    Text { text: String, finalized: bool },
    NoSpeech(String),
    Error(String),
    Saved,
    Unloaded,
    Cancelled,
    Heartbeat,
}
pub fn emit(event: Event) {
    if let Ok(json) = serde_json::to_string(&event) {
        let mut out = io::stdout().lock();
        let _ = writeln!(out, "{json}");
        let _ = out.flush();
    }
}
pub fn tail(text: &str) -> String {
    let start = text.char_indices().rev().nth(359).map_or(0, |(i, _)| i);
    let text = &text[start..];
    if start > 0 {
        format!("…{text}")
    } else {
        text.to_string()
    }
}
#[derive(Default)]
struct AudioSummary {
    samples: usize,
    peak: f32,
}
impl AudioSummary {
    fn observe(&mut self, frame: &[f32]) {
        self.samples += frame.len();
        for &sample in frame {
            self.peak = self.peak.max(sample.abs());
        }
    }
}
fn final_event(text: &str, audio: &AudioSummary, microphone: Option<&str>) -> Event {
    if !text.trim().is_empty() {
        return Event::Text {
            text: tail(text),
            finalized: true,
        };
    }
    let input = microphone.unwrap_or("Entrada del sistema");
    let message = if audio.samples == 0 {
        format!("{input} no envió audio. Espera «Habla ahora» y revisa la entrada")
    } else if audio.peak < 0.0001 {
        format!("{input}: señal prácticamente silenciosa. Revisa mute, ganancia y conexión")
    } else {
        format!(
            "Se recibió audio durante {:.1} s, pero no se reconocieron palabras",
            audio.samples as f64 / 16_000.0
        )
    };
    Event::NoSpeech(message)
}

// Solo el proceso hijo toca el micrófono y el motor nativo.
pub fn run_worker(path: &Path, microphone: Option<&str>) -> Result<()> {
    transcribe_cpp::disable_logging();
    let (tx, commands) = mpsc::sync_channel(8);
    let input = std::thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            let Ok(line) = line else {
                break;
            };
            if line.len() > 32 || tx.send(line).is_err() {
                break;
            }
        }
    });
    let result = (|| -> Result<()> {
        let mut model: Option<Model> = None;
        while let Ok(command) = commands.recv() {
            if command == "load" || command == "start" {
                if model.is_none() {
                    emit(Event::Status("Cargando modelo…".into()));
                    if !model::ready(path) {
                        return Err(
                            "El modelo cambió o no está verificado. Usa Preparar modelo".into()
                        );
                    }
                    model = Some(Model::load(path)?);
                    emit(Event::ModelLoaded);
                } else if command == "load" {
                    emit(Event::ModelLoaded);
                }
            }
            if command != "start" {
                continue;
            }
            // Si se soltó la tecla durante la carga, no abrir el micrófono después.
            match commands.try_recv() {
                Ok(command) if command == "stop" => {
                    emit(Event::Cancelled);
                    continue;
                }
                Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
                _ => {}
            }
            let loaded = model.as_ref().ok_or("Modelo no disponible")?;
            if !loaded.capabilities().supports_streaming {
                return Err("El modelo no admite streaming".into());
            }
            // Captura desde que el modelo está disponible; la cola conserva el comienzo
            // mientras se prepara la sesión nativa, sin abrir entrada durante precarga.
            let mut capture = Capture::start(microphone).map_err(|error| {
                format!("No se pudo abrir el micrófono: {error}. Revisa la entrada y su permiso")
            })?;
            let mut session = loaded.session_with(&SessionOptions {
                n_threads: 2,
                ..Default::default()
            })?;
            let run = RunOptions {
                language: Some("es-ES".into()),
                timestamps: TimestampKind::None,
                ..Default::default()
            };
            let options = StreamOptions {
                family: Some(StreamExtension::ParakeetStream(ParakeetStreamOptions {
                    att_context_right: Some(3),
                })),
                ..Default::default()
            };
            let mut stream = session.stream(&run, &options)?;
            let mut audio = AudioSummary::default();
            let mut listening = false;
            let started = Instant::now();
            let mut heartbeat = Instant::now();
            let mut stopping = false;
            let mut final_audio = None;
            loop {
                match commands.try_recv() {
                    Ok(command) if command == "stop" => stopping = true,
                    Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
                    _ => {}
                }
                if started.elapsed() >= Duration::from_secs(300) {
                    stopping = true;
                }
                if stopping && final_audio.is_none() {
                    final_audio = Some(capture.stop());
                }
                capture.check()?;
                match capture.frames.recv_timeout(Duration::from_millis(25)) {
                    Ok(frame) => {
                        if !listening {
                            emit(Event::Listening);
                            listening = true;
                        }
                        audio.observe(&frame);
                        let update = stream.feed(&frame)?;
                        if update.result_changed {
                            emit(Event::Text {
                                text: tail(&stream.text().display()),
                                finalized: false,
                            });
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) if stopping => break,
                    Err(mpsc::RecvTimeoutError::Timeout) if stopping => break,
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        return Err("Captura de audio desconectada".into());
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
                if heartbeat.elapsed() >= Duration::from_secs(1) {
                    emit(Event::Heartbeat);
                    heartbeat = Instant::now();
                }
            }
            if let Some(tail) = final_audio
                && !tail.is_empty()
            {
                audio.observe(&tail);
                stream.feed(&tail)?;
            }
            stream.finalize()?;
            emit(final_event(&stream.text().display(), &audio, microphone));
        }
        Ok(())
    })();
    // En fallo, el padre termina el proceso; no esperar stdin abierto aquí.
    if result.is_ok() {
        input.join().map_err(|_| "Entrada del worker inválida")?;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_result_distinguishes_missing_silent_and_unrecognized_audio() {
        let mut audio = AudioSummary::default();
        let Event::NoSpeech(message) = final_event("", &audio, Some("USB Audio CODEC")) else {
            panic!("Un resultado vacío debe explicar la entrada recibida");
        };
        assert!(message.contains("no envió audio"));
        audio.observe(&vec![0.0; 16_000]);
        let Event::NoSpeech(message) = final_event("", &audio, Some("USB Audio CODEC")) else {
            panic!()
        };
        assert!(message.contains("prácticamente silenciosa"));
        assert!(message.contains("USB Audio CODEC"));
        audio.observe(&vec![0.1; 16_000]);
        let Event::NoSpeech(message) = final_event("", &audio, None) else {
            panic!()
        };
        assert!(message.contains("2.0 s"));
        assert!(message.contains("no se reconocieron palabras"));
        assert!(
            matches!(final_event("Hola", &audio, None), Event::Text { text, finalized: true } if text == "Hola")
        );
    }
    #[test]
    fn display_tail_is_bounded_and_utf8_safe() {
        let text = "á🦀".repeat(500);
        let clipped = tail(&text);
        assert_eq!(clipped.chars().count(), 361);
        assert!(clipped.starts_with('…'));
        assert_eq!(tail("Hola"), "Hola");
    }
}
