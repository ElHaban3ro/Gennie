use super::*;
use crate::{PresenceModule, assistants};
use gennie_core::AppModule;
use image::ImageEncoder;
use image::{DynamicImage, Rgba, RgbaImage};

fn setup(db: &Database) {
    let module = PresenceModule::default();
    db.migrate(module.name(), module.migrations()).unwrap();
}

fn picture(format: ImageFormat, width: u32, height: u32) -> Vec<u8> {
    let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(
        width,
        height,
        Rgba([40, 110, 180, 120]),
    ));
    let mut encoded = Cursor::new(Vec::new());
    if format == ImageFormat::Jpeg {
        image.to_rgb8().write_to(&mut encoded, format).unwrap();
    } else {
        image.write_to(&mut encoded, format).unwrap();
    }
    encoded.into_inner()
}

#[test]
fn accepted_formats_are_copied_to_bounded_png_preserving_aspect_and_alpha() {
    for format in [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::WebP] {
        let encoded = picture(format, 800, 400);
        let normalized = normalize(&encoded).unwrap();
        assert_eq!(image::guess_format(&normalized).unwrap(), ImageFormat::Png);
        let decoded = image::load_from_memory(&normalized).unwrap().to_rgba8();
        assert_eq!(decoded.dimensions(), (256, 128));
        assert!(normalized.len() < 1048576);
        if format != ImageFormat::Jpeg {
            assert_eq!(decoded.get_pixel(0, 0)[3], 120);
        }
    }
}

#[test]
fn exif_orientation_is_applied_to_pixels_and_metadata_is_not_copied() {
    let image = image::RgbImage::from_pixel(800, 400, image::Rgb([40, 110, 180]));
    let mut bytes = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new(&mut bytes);
    // TIFF little-endian con orientación 6 (90°), sin archivo ni metadatos personales.
    encoder
        .set_exif_metadata(
            b"II*\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0".to_vec(),
        )
        .unwrap();
    encoder.encode_image(&image).unwrap();
    let output = normalize(&bytes).unwrap();
    let mut decoder = ImageReader::with_format(Cursor::new(&output), ImageFormat::Png)
        .into_decoder()
        .unwrap();
    assert_eq!(decoder.dimensions(), (128, 256));
    assert!(decoder.exif_metadata().unwrap().is_none());
    assert!(decoder.icc_profile().unwrap().is_none());
}

#[test]
fn corrupt_unsupported_oversized_and_large_dimension_images_do_not_change_profile() {
    let db = Database::memory().unwrap();
    setup(&db);
    let original = picture(ImageFormat::Png, 12, 12);
    save_profile(&db, "gennie", "Mi Gennie", Some(&original)).unwrap();
    let before = assistants::load(&db, "gennie").unwrap();
    let pixels = load(&db, "gennie", 1).unwrap();
    for invalid in [
        b"<svg></svg>".to_vec(),
        vec![0; MAX_SOURCE_BYTES + 1],
        picture(ImageFormat::Png, 4097, 1),
        original[..original.len() / 2].to_vec(),
    ] {
        assert!(save_profile(&db, "gennie", "No guardar", Some(&invalid)).is_err());
        assert_eq!(assistants::load(&db, "gennie").unwrap(), before);
        assert_eq!(load(&db, "gennie", 1).unwrap(), pixels);
    }
    assert!(save_profile(&db, "gennie", " \n ", Some(&original)).is_err());
    assert!(save_profile(&db, "missing", "Nombre", Some(&original)).is_err());
    assert_eq!(assistants::load(&db, "gennie").unwrap(), before);
}

#[test]
fn keep_replace_reset_and_failed_commit_are_atomic_and_revisions_reject_stale_reads() {
    let db = Database::memory().unwrap();
    setup(&db);
    let image = picture(ImageFormat::Png, 80, 40);
    save_profile(&db, "gennie", "Primer nombre", Some(&image)).unwrap();
    assert_eq!(
        assistants::load(&db, "gennie").unwrap().avatar_revision,
        Some(1)
    );
    let first = load(&db, "gennie", 1).unwrap();
    save_profile(&db, "gennie", "Solo nombre", None).unwrap();
    assert_eq!(load(&db, "gennie", 1).unwrap(), first);
    let before = assistants::load(&db, "gennie").unwrap();
    db.with_connection(|c| { c.execute_batch("CREATE TRIGGER reject_profile BEFORE UPDATE ON presence_assistants BEGIN SELECT RAISE(ABORT,'fixture'); END;")?; Ok(()) }).unwrap();
    assert!(save_profile(&db, "gennie", "Fallo", Some(&[])).is_err());
    assert_eq!(assistants::load(&db, "gennie").unwrap(), before);
    assert_eq!(load(&db, "gennie", 1).unwrap(), first);
    db.with_connection(|c| {
        c.execute_batch("DROP TRIGGER reject_profile;")?;
        Ok(())
    })
    .unwrap();
    save_profile(&db, "gennie", "Predeterminado", Some(&[])).unwrap();
    assert_eq!(
        assistants::load(&db, "gennie").unwrap().avatar_revision,
        None
    );
    assert!(load(&db, "gennie", 1).is_err());
    save_profile(&db, "gennie", "Nueva imagen", Some(&image)).unwrap();
    assert_eq!(
        assistants::load(&db, "gennie").unwrap().avatar_revision,
        Some(3)
    );
    assert!(load(&db, "gennie", 1).is_err());
    assert_eq!(load(&db, "gennie", 3).unwrap(), first);
}

#[test]
fn migration_and_reopen_preserve_existing_identity_sessions_messages_and_quotes() {
    let directory = std::env::temp_dir().join(format!(
        "gennie-avatar-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path = directory.join("fixture.sqlite3");
    let db = Database::open(&path, 50).unwrap();
    let module = PresenceModule::default();
    db.migrate(module.name(), &module.migrations()[..13])
        .unwrap();
    db.with_connection(|c| {
        c.execute_batch("INSERT INTO presence_conversations(id,title) VALUES(1,'Conservar');
            INSERT INTO presence_current_conversation VALUES(1,1);
            INSERT INTO presence_history(id,conversation_id,speaker,text) VALUES(1,1,'user','Original'),(2,1,'user','Respuesta');
            INSERT INTO presence_message_replies(message_id,source_message_id,speaker,text,quoted_created_at,truncated) VALUES(2,1,'user','Original','2026-10-28T00:00:00.000Z',0);")?;
        Ok(())
    }).unwrap();
    let before = crate::history::legacy_fixture::load(&db, None).unwrap();
    setup(&db);
    assert_eq!(
        assistants::load(&db, "gennie").unwrap().avatar_revision,
        None
    );
    save_profile(
        &db,
        "gennie",
        "Mi asistente",
        Some(&picture(ImageFormat::Png, 32, 16)),
    )
    .unwrap();
    let pixels = load(&db, "gennie", 1).unwrap();
    db.close().unwrap();
    let db = Database::open(&path, 50).unwrap();
    setup(&db);
    assert_eq!(
        assistants::load(&db, "gennie").unwrap().name,
        "Mi asistente"
    );
    assert_eq!(load(&db, "gennie", 1).unwrap(), pixels);
    let after = crate::history::load(&db, None).unwrap();
    assert_eq!(after.messages, before.messages);
    assert_eq!(after.conversations, before.conversations);
    assert_eq!(after.active_id, before.active_id);
    assert_eq!(
        assistants::page(&db, "gennie", None, 60, 24000)
            .unwrap()
            .messages
            .last()
            .unwrap()
            .quote
            .as_ref()
            .unwrap()
            .source_message_id,
        Some(1)
    );
    db.with_connection(|c| {
        assert!(
            c.prepare("PRAGMA foreign_key_check")?
                .query([])?
                .next()?
                .is_none()
        );
        Ok(())
    })
    .unwrap();
    db.close().unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}
