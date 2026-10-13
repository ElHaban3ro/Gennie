// Sonda explícita: audio sintético f32 LE mono 16 kHz, nunca micrófono.
use soc_core::Result;
use std::{
    io::Read,
    time::{Duration, Instant},
};
use transcribe_cpp::{
    Model, ParakeetStreamOptions, RunOptions, SessionOptions, StreamExtension, StreamOptions,
    TimestampKind,
};
fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let model_path = args.next().ok_or("Falta modelo")?;
    let audio_path = args.next().ok_or("Falta audio f32")?;
    let mut data = Vec::new();
    std::fs::File::open(audio_path)?
        .take(16_000 * 4 * 30 + 1)
        .read_to_end(&mut data)?;
    if data.len() % 4 != 0 || data.len() > 16_000 * 4 * 30 {
        return Err("Sonda admite hasta 30 s de f32 LE".into());
    }
    let audio: Vec<f32> = data
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    transcribe_cpp::disable_logging();
    let started = Instant::now();
    let backend = if args.next().is_some_and(|s| s == "cpu") {
        transcribe_cpp::Backend::Cpu
    } else {
        transcribe_cpp::Backend::Auto
    };
    let model = Model::load_with(
        model_path,
        &transcribe_cpp::ModelOptions {
            backend,
            ..Default::default()
        },
    )?;
    println!(
        "loaded_ms={} backend={} pid={}",
        started.elapsed().as_millis(),
        model.backend(),
        std::process::id()
    );
    std::thread::sleep(Duration::from_secs(3));
    let mut session = model.session_with(&SessionOptions {
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
    let start = Instant::now();
    let mut updates = 0;
    for frame in audio.chunks(1600) {
        if stream.feed(frame)?.result_changed {
            updates += 1;
            println!("partial={}", stream.text().display());
        }
    }
    stream.finalize()?;
    println!(
        "final={} updates={} audio_ms={} compute_ms={}",
        stream.text().display(),
        updates,
        audio.len() * 1000 / 16000,
        start.elapsed().as_millis()
    );
    drop(stream);
    drop(session);
    drop(model);
    println!("released");
    std::thread::sleep(Duration::from_secs(2));
    Ok(())
}
