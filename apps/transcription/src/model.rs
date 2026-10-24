use crate::Event;
use gennie_core::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

pub const FILE_NAME: &str = "nemotron-3.5-asr-streaming-0.6b-Q8_0.gguf";
pub const MODEL_BYTES: u64 = 751_094_240;
pub const MODEL_SHA256: &str = "b94545b313b3223fda7b2857a52681da813935c2127643d1e9ff0c23d988089c";
pub const MODEL_REVISION: &str = "8139c4ec14bdc45c361adf8d57c27c28e7478272";
pub fn path() -> Result<PathBuf> {
    let home =
        std::env::var_os("HOME").ok_or("No se encontró la carpeta personal para el modelo")?;
    Ok(PathBuf::from(home)
        .join("Library/Application Support/AIModels/gguf")
        .join(FILE_NAME))
}
#[derive(Serialize, Deserialize)]
struct Verification {
    sha256: String,
    fingerprint: String,
}
fn fingerprint(path: &Path) -> Option<String> {
    let metadata = path.metadata().ok()?;
    let modified = metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    let mut hash = Sha256::new();
    hash.update(metadata.len().to_le_bytes());
    hash.update(modified.to_le_bytes());
    Some(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}
pub fn ready(path: &Path) -> bool {
    let marker = fs::read_to_string(path.with_extension("sha256"))
        .ok()
        .and_then(|s| serde_json::from_str::<Verification>(&s).ok());
    path.metadata().is_ok_and(|m| m.len() == MODEL_BYTES)
        && marker
            .is_some_and(|m| m.sha256 == MODEL_SHA256 && Some(m.fingerprint) == fingerprint(path))
}
pub fn verify(path: &Path) -> Result<()> {
    if path.metadata()?.len() != MODEL_BYTES {
        return Err("Tamaño del modelo incorrecto".into());
    }
    verify_reader(File::open(path)?, MODEL_SHA256)
}
fn verify_reader(mut file: impl Read, expected: &str) -> Result<()> {
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    if hash
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
        != expected
    {
        return Err("La verificación SHA-256 del modelo falló".into());
    }
    Ok(())
}
pub(crate) fn partial(path: &Path, pid: u32) -> PathBuf {
    path.with_extension(format!("partial-{pid}"))
}
struct PartialFile(PathBuf);
impl Drop for PartialFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
// Este helper vive en un proceso cancelable: el event loop no hace red ni hashing.
pub fn prepare(path: &Path, mut notify: impl FnMut(Event)) -> Result<()> {
    fs::create_dir_all(path.parent().ok_or("Ruta de modelo inválida")?)?;
    let existing_valid = if path.is_file() {
        notify(Event::Status("Verificando Nemotron…".into()));
        ready(path) || verify(path).is_ok()
    } else {
        false
    };
    if !existing_valid {
        let partial = PartialFile(partial(path, std::process::id()));
        let url = format!(
            "https://huggingface.co/handy-computer/nemotron-3.5-asr-streaming-0.6b-gguf/resolve/{MODEL_REVISION}/{FILE_NAME}"
        );
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .timeout(Duration::from_secs(900))
            .build()?;
        let mut response = client.get(url).send()?.error_for_status()?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial.0)?;
        let mut buffer = [0u8; 65536];
        let mut size = 0u64;
        let mut previous = 101;
        loop {
            let n = response.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            size += n as u64;
            if size > MODEL_BYTES {
                return Err("Descarga excede el tamaño esperado".into());
            }
            file.write_all(&buffer[..n])?;
            let percent = (size * 100 / MODEL_BYTES) as u8;
            if percent != previous {
                notify(Event::Download(percent));
                previous = percent;
            }
        }
        file.sync_all()?;
        drop(file);
        notify(Event::Status("Verificando Nemotron…".into()));
        verify(&partial.0)?;
        fs::rename(&partial.0, path)?;
    }
    let verification = Verification {
        sha256: MODEL_SHA256.into(),
        fingerprint: fingerprint(path).ok_or("No se pudo identificar el modelo verificado")?,
    };
    fs::write(
        path.with_extension("sha256"),
        serde_json::to_vec(&verification)?,
    )?;
    notify(Event::Ready);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn integrity_rejects_modified_content() {
        let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert!(verify_reader(&b"abc"[..], expected).is_ok());
        assert!(verify_reader(&b"abd"[..], expected).is_err());
        assert!(verify_reader(&b""[..], expected).is_err());
    }
}
