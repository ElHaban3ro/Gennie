use soc_db::{Database, Result, params};
pub fn load(db: &Database) -> Result<bool> {
    db.with_connection(|c| {
        Ok(c.query_row(
            "SELECT reduced_motion FROM presence_preferences WHERE id=1",
            [],
            |r| r.get(0),
        )?)
    })
}
pub fn save(db: &Database, reduced: bool) -> Result<()> {
    db.with_connection(|c|{c.execute("UPDATE presence_preferences SET reduced_motion=?1, updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=1",params![reduced])?;Ok(())})
}
pub(crate) fn load_appearance(db: &Database) -> soc_core::Result<crate::appearance::Settings> {
    let value = db.with_connection(|c| {
        Ok(c.query_row(
            "SELECT blur_radius, saturation_percent, brightness_percent, tint_percent, border_percent, border_width_hundredths, transition_ms FROM presence_appearance WHERE id=1",
            [],
            |r| Ok(crate::appearance::Settings {
                blur_radius: r.get(0)?, saturation_percent: r.get(1)?, brightness_percent: r.get(2)?,
                tint_percent: r.get(3)?, border_percent: r.get(4)?, border_width_hundredths: r.get(5)?, transition_ms: r.get(6)?,
            }),
        )?)
    })?;
    value.validate()?;
    Ok(value)
}

pub(crate) fn save_appearance(
    db: &Database,
    value: crate::appearance::Settings,
) -> soc_core::Result<()> {
    value.validate()?;
    db.with_connection(|c| {
        c.execute(
            "UPDATE presence_appearance SET blur_radius=?1, saturation_percent=?2, brightness_percent=?3, tint_percent=?4, border_percent=?5, border_width_hundredths=?6, transition_ms=?7, updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=1",
            params![value.blur_radius, value.saturation_percent, value.brightness_percent, value.tint_percent, value.border_percent, value.border_width_hundredths, value.transition_ms],
        )?;
        Ok(())
    })?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use soc_core::AppModule;
    #[test]
    fn appearance_default_upgrade_preserves_custom_profile() {
        let migrations = crate::PresenceModule::default().migrations();
        for (version, custom) in [(4, false), (4, true), (5, false), (5, true)] {
            let path = std::env::temp_dir().join(format!(
                "soc-appearance-upgrade-{}-{version}-{custom}.sqlite3",
                std::process::id()
            ));
            let db = Database::open(&path, 50).unwrap();
            db.migrate("presence", &migrations[..version]).unwrap();
            let mut before = load_appearance(&db).unwrap();
            if custom {
                before.blur_radius = 2;
                save_appearance(&db, before).unwrap();
            }
            db.migrate("presence", migrations).unwrap();
            let expected = if custom {
                before
            } else {
                crate::appearance::Settings::default()
            };
            assert_eq!(load_appearance(&db).unwrap(), expected);
            db.close().unwrap();
            let db = Database::open(&path, 50).unwrap();
            assert_eq!(load_appearance(&db).unwrap(), expected);
            db.close().unwrap();
            std::fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn preference_survives_reopening() {
        let path = std::env::temp_dir().join(format!(
            "soc-preferences-{}-{:?}.sqlite3",
            std::process::id(),
            std::thread::current().id()
        ));
        let db = Database::open(&path, 50).unwrap();
        db.migrate("presence", crate::PresenceModule::default().migrations())
            .unwrap();
        assert!(!load(&db).unwrap());
        assert_eq!(
            load_appearance(&db).unwrap(),
            crate::appearance::Settings::default()
        );
        save(&db, true).unwrap();
        let appearance = crate::appearance::Settings {
            blur_radius: 3,
            saturation_percent: 125,
            brightness_percent: 95,
            tint_percent: 12,
            border_percent: 50,
            border_width_hundredths: 150,
            transition_ms: 70,
        };
        save_appearance(&db, appearance).unwrap();
        let invalid = crate::appearance::Settings {
            blur_radius: 31,
            ..appearance
        };
        assert!(save_appearance(&db, invalid).is_err());
        assert_eq!(load_appearance(&db).unwrap(), appearance);
        assert!(
            db.with_connection(|c| {
                c.execute(
                    "UPDATE presence_appearance SET tint_percent=80 WHERE id=1",
                    [],
                )?;
                Ok(())
            })
            .is_err()
        );
        db.close().unwrap();
        let db = Database::open(&path, 50).unwrap();
        db.migrate("presence", crate::PresenceModule::default().migrations())
            .unwrap();
        assert!(load(&db).unwrap());
        assert_eq!(load_appearance(&db).unwrap(), appearance);
        db.close().unwrap();
        std::fs::remove_file(path).unwrap();
    }
}
