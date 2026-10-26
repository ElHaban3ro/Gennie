use gennie_backend::startup;
fn main() {
    let mut arguments = std::env::args_os().skip(1);
    if let Some(flag) = arguments.next() {
        if flag == "--mcp" {
            let result = (|| -> gennie_core::Result<()> {
                let socket = arguments.next().ok_or("Falta socket MCP")?;
                if arguments.next().is_some() {
                    return Err("Argumentos sobrantes".into());
                }
                gennie_mcp::bridge(std::path::Path::new(&socket))
            })();
            if result.is_err() {
                eprintln!("No se pudo completar la conexión MCP local");
                std::process::exit(1);
            }
            return;
        }
        if flag == "--voice-review" {
            if arguments.next().is_some() {
                eprintln!("Argumentos sobrantes");
                std::process::exit(1);
            }
            if let Err(error) = startup::voice_review() {
                eprintln!("Prueba de voz: {error}");
                std::process::exit(1);
            }
            return;
        }
        if flag == "--decide" || flag == "--command" {
            let result = (|| -> gennie_core::Result<()> {
                let text = arguments
                    .next()
                    .ok_or("Falta frase")?
                    .into_string()
                    .map_err(|_| "Frase inválida")?;
                if arguments.next().is_some() {
                    return Err("Argumentos sobrantes".into());
                }
                startup::command(&text, flag == "--command")
            })();
            if let Err(error) = result {
                eprintln!("Control local fallido: {error}");
                std::process::exit(1);
            }
            return;
        }
        let helper = flag == "--prepare-model" || flag == "--transcription-worker";
        if helper {
            let result = (|| -> gennie_core::Result<()> {
                let path = arguments
                    .next()
                    .map(std::path::PathBuf::from)
                    .ok_or("Falta ruta del modelo")?;
                if flag == "--prepare-model" {
                    gennie_transcription::model::prepare(&path, gennie_transcription::worker_emit)
                } else {
                    let microphone = match arguments.next() {
                        Some(flag) if flag == "--microphone" => Some(
                            arguments
                                .next()
                                .ok_or("Falta micrófono")?
                                .into_string()
                                .map_err(|_| "Micrófono inválido")?,
                        ),
                        Some(_) => return Err("Argumento del worker no reconocido".into()),
                        None => None,
                    };
                    if arguments.next().is_some() {
                        return Err("Argumentos sobrantes".into());
                    }
                    gennie_transcription::run_worker(&path, microphone.as_deref())
                }
            })();
            if let Err(error) = result {
                // Mensaje dirigido al globo, nunca audio ni transcripción en logs.
                let message = if flag == "--prepare-model" {
                    "No se pudo descargar/verificar Nemotron. Revisa conexión y espacio; reintenta desde Transcripción".to_string()
                } else {
                    error.to_string()
                };
                gennie_transcription::worker_emit(gennie_transcription::Event::Error(message));
                std::process::exit(1);
            }
            return;
        }
        eprintln!("Argumento no reconocido");
        std::process::exit(1);
    }
    eprintln!(
        "Inicia el escritorio con ./scripts/dev.sh; este binario ejecuta los helpers de Rust."
    );
}
