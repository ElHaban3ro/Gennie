//! Copia local de perfil, decodificada fuera del lock SQLite y sin rutas de usuario.
use gennie_core::Result;
use gennie_db::{Database, params};
use image::{ImageDecoder, ImageFormat, ImageReader, Limits};
use std::io::Cursor;

pub const MAX_SOURCE_BYTES: usize = 5 * 1024 * 1024;

/// None conserva la imagen; un vector vacío restaura el pingüino predeterminado.
pub fn save_profile(db: &Database, id: &str, name: &str, avatar: Option<&[u8]>) -> Result<()> {
    let name = crate::assistants::validate_name(name)?;
    // La decodificación no mantiene la conexión bloqueada ni una transacción abierta.
    let normalized = avatar
        .filter(|bytes| !bytes.is_empty())
        .map(normalize)
        .transpose()?;
    db.with_connection(|c| {
        let tx = c.transaction()?;
        let changed = tx.execute(
            "UPDATE presence_assistants SET name=?2,
             avatar_png=CASE WHEN ?3 THEN ?4 ELSE avatar_png END,
             avatar_revision=avatar_revision+?3,
             updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1",
            params![id, name, avatar.is_some(), normalized],
        )?;
        if changed != 1 {
            return Err("El asistente ya no está disponible".into());
        }
        tx.commit()?;
        Ok(())
    })
}

/// Lectura bajo demanda de la revisión solicitada; nunca se incluyen píxeles en snapshots.
pub fn load(db: &Database, id: &str, revision: i64) -> Result<Vec<u8>> {
    db.with_connection(|c| {
        let mut statement = c.prepare(
            "SELECT avatar_png FROM presence_assistants WHERE id=?1 AND avatar_revision=?2 AND avatar_png IS NOT NULL",
        )?;
        let mut rows = statement.query(params![id, revision])?;
        let row = rows.next()?.ok_or("La imagen cambió o ya no está disponible; vuelve a abrir el perfil")?;
        Ok(row.get(0)?)
    })
}

pub fn normalize(bytes: &[u8]) -> Result<Vec<u8>> {
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err("La imagen debe ocupar como máximo 5 MiB".into());
    }
    let format =
        image::guess_format(bytes).map_err(|_| "Elige una imagen PNG, JPEG o WebP válida")?;
    if !matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP
    ) {
        return Err("Elige una imagen PNG, JPEG o WebP".into());
    }
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader
        .into_decoder()
        .map_err(|_| "Imagen inválida o mayor que 4096 × 4096 píxeles")?;
    let orientation = decoder
        .orientation()
        .map_err(|_| "No se pudo leer la orientación de la imagen")?;
    let mut image = image::DynamicImage::from_decoder(decoder)
        .map_err(|_| "No se pudo decodificar la imagen")?;
    image.apply_orientation(orientation);
    let image = image.thumbnail(256, 256).to_rgba8();
    let mut output = Cursor::new(Vec::new());
    image
        .write_to(&mut output, ImageFormat::Png)
        .map_err(|_| "No se pudo preparar la imagen de perfil")?;
    Ok(output.into_inner())
}

#[cfg(test)]
mod tests;
