//! Enmarca líneas sin volver a escanear/decodificar un resultado binario grande.
use super::*;
#[derive(Default)]
pub(super) struct Lines(Vec<u8>);
impl Lines {
    pub fn push(
        &mut self,
        bytes: &[u8],
        limit: usize,
        mut accept: impl FnMut(&[u8]) -> Result<()>,
    ) -> Result<()> {
        for part in bytes.split_inclusive(|b| *b == b'\n') {
            self.0.extend_from_slice(part);
            if self.0.len() > limit {
                return Err("Mensaje MCP demasiado grande".into());
            }
            if part.last() == Some(&b'\n') {
                let line = std::mem::take(&mut self.0);
                accept(&line)?;
            }
        }
        Ok(())
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
