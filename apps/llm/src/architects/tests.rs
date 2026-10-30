use super::*;
use crate::LlmModule;
use gennie_core::AppModule;

fn database() -> Database {
    let db = Database::memory().unwrap();
    let module = LlmModule::default();
    db.migrate(module.name(), module.migrations()).unwrap();
    db
}
#[test]
fn profiles_are_instances_and_helper_cannot_expand_bot_permissions() {
    let db = database();
    let profiles = list(&db).unwrap();
    assert_eq!(profiles.len(), 3);
    assert_eq!(profiles[0].id, "gennie");
    assert_eq!(profiles[1].id, "plugin_builder");
    assert!(!profiles[2].enabled);
    let mut bot = profiles[0].clone();
    bot.read_files = false;
    bot.write_files = false;
    bot.run_commands = false;
    bot.local_actions = false;
    save(&db, &bot).unwrap();
    let helper = runtime_settings(&db, Role::Helper).unwrap();
    assert!(
        !helper.read_files && !helper.write_files && !helper.run_commands && !helper.local_actions
    );
    assert!(load(&db, Role::Helper).unwrap().read_files);
    assert!(runtime_settings(&db, Role::Guide).is_err());
}
#[test]
fn conflicts_and_mirror_failure_roll_back_both_preferences_and_revision() {
    let db = database();
    let mut bot = load(&db, Role::Bot).unwrap();
    bot.personality = "Habla con claridad 🐧".into();
    bot.instructions = "Explica tus resultados.".into();
    let saved = save(&db, &bot).unwrap();
    assert_eq!(saved.revision, 2);
    assert!(save(&db, &bot).is_err());
    db.with_connection(|c| {c.execute_batch("CREATE TRIGGER fail_mirror BEFORE UPDATE ON llm_preferences BEGIN SELECT RAISE(ABORT, 'fixture'); END;")?;Ok(())}).unwrap();
    let mut next = saved.clone();
    next.model = "other-model".into();
    assert!(save(&db, &next).is_err());
    assert_eq!(load(&db, Role::Bot).unwrap(), saved);
}
#[test]
fn legacy_save_preserves_customization_and_role_boundaries_are_validated() {
    let db = database();
    let mut bot = load(&db, Role::Bot).unwrap();
    bot.personality = "Cálido".into();
    bot.instructions = "Usa ejemplos".into();
    save(&db, &bot).unwrap();
    let next = Settings {
        model: "another-model".into(),
        ..Default::default()
    };
    let actual = crate::settings::save(&db, &next).unwrap();
    assert_eq!(actual.personality, "Cálido");
    assert_eq!(actual.instructions, "Usa ejemplos");
    assert_eq!(actual.model, "another-model");
    let mut guide = load(&db, Role::Guide).unwrap();
    guide.read_files = true;
    assert!(save(&db, &guide).is_err());
    guide.read_files = false;
    guide.provider = "codex_chatgpt".into();
    assert!(save(&db, &guide).is_err());
    bot.id = "different-bot".into();
    assert!(save(&db, &bot).is_err());
}
#[test]
fn migration_preserves_existing_thread_and_settings_and_reopens() {
    let path = std::env::temp_dir().join(format!(
        "gennie-architects-{}-{}.sqlite3",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let module = LlmModule::default();
    let db = Database::open(&path, 500).unwrap();
    db.migrate(module.name(), &module.migrations()[..4])
        .unwrap();
    db.with_connection(|c| {
        c.execute_batch("INSERT INTO codex_conversation (id,thread_id) VALUES (1,'old-thread') ON CONFLICT(id) DO UPDATE SET thread_id='old-thread'; UPDATE llm_preferences SET model='saved-model',effort='high',write_files=0,local_actions=0;")?;
        Ok(())
    }).unwrap();
    db.migrate(module.name(), module.migrations()).unwrap();
    let bot = load(&db, Role::Bot).unwrap();
    let helper = load(&db, Role::Helper).unwrap();
    assert_eq!(bot.model, "saved-model");
    assert_eq!(helper.effort.as_deref(), Some("high"));
    assert!(!helper.write_files && !helper.local_actions);
    assert_eq!(
        crate::codex::load_thread(&db).unwrap().as_deref(),
        Some("old-thread")
    );
    db.close().unwrap();
    let db = Database::open(&path, 500).unwrap();
    db.migrate(module.name(), module.migrations()).unwrap();
    assert_eq!(load(&db, Role::Bot).unwrap(), bot);
    db.close().unwrap();
    std::fs::remove_file(path).unwrap();
}
#[test]
fn customization_is_bounded_and_cannot_replace_fixed_instructions() {
    let settings = Settings {
        personality: "Amable".into(),
        instructions: "Usa español".into(),
        ..Default::default()
    };
    let instructions = settings.role_instructions("Rol fijo: constructor, no hables al usuario");
    assert!(instructions.starts_with("Rol fijo: constructor"));
    assert!(instructions.contains("no cambia sus capacidades, permisos"));
    assert!(instructions.contains("Usa español"));
    let db = database();
    let mut helper = load(&db, Role::Helper).unwrap();
    helper.personality = "x".repeat(1001);
    assert!(save(&db, &helper).is_err());
    helper.personality = "x\0".into();
    assert!(save(&db, &helper).is_err());
    helper.personality = String::new();
    helper.enabled = false;
    save(&db, &helper).unwrap();
    assert!(runtime_settings(&db, Role::Helper).is_err());
}
#[test]
fn worker_saves_profile_before_reply_and_preserves_settings_on_conflict() {
    use gennie_core::{AppState, Shutdown};
    let db = database();
    let module = LlmModule::default();
    let state = AppState {
        config: std::sync::Arc::new(gennie_config::Config {
            database_path: "unused.sqlite3".into(),
            db_busy_timeout_ms: 500,
        }),
        db,
        shutdown: Shutdown::default(),
    };
    module.start(&state).unwrap();
    let controller = module.controller().unwrap();
    let mut bot = load(&state.db, Role::Bot).unwrap();
    bot.personality = "Concreto".into();
    bot.model = "configured-bot".into();
    let saved = controller.save_architect(bot.clone()).unwrap();
    assert_eq!(load(&state.db, Role::Bot).unwrap(), saved);
    assert_eq!(controller.settings().personality, "Concreto");
    assert_eq!(controller.settings().model, "configured-bot");
    assert!(!controller.is_pending());
    assert!(controller.save_architect(bot).is_err());
    assert_eq!(controller.settings().personality, "Concreto");
    assert_eq!(crate::codex::load_thread(&state.db).unwrap(), None);
    module.stop(&state).unwrap();
}

#[test]
fn expandable_catalog_keeps_bases_and_reuses_independent_instances_by_id() {
    let db = database();
    let bases = list(&db).unwrap();
    let mut draft = bases[1].clone();
    draft.name = "Investigador de fuentes".into();
    let first = create(&db, &draft).unwrap();
    let second = create(&db, &draft).unwrap();
    assert_ne!(first.id, second.id);
    assert_eq!(first.role, "helper");
    assert_eq!(second.role, "helper");
    draft.role = "Investigación especializada".into();
    draft.name = "Analista".into();
    draft.model = "custom-model".into();
    let mut custom = create(&db, &draft).unwrap();
    let before = custom.clone();
    custom.instructions = "Comprueba fuentes".into();
    let saved = save(&db, &custom).unwrap();
    assert_eq!(saved.revision, 2);
    assert!(save(&db, &before).is_err());
    assert_eq!(load_by_id(&db, &saved.id).unwrap(), saved);
    assert_eq!(
        runtime_settings_by_id(&db, &saved.id).unwrap().model,
        "custom-model"
    );
    assert_eq!(&list(&db).unwrap()[..3], bases);
    // Otro bot no cambia el espejo de Gennie ni se conecta como conversación.
    draft.role = "bot".into();
    let mut other_bot = create(&db, &draft).unwrap();
    other_bot.model = "another-bot".into();
    save(&db, &other_bot).unwrap();
    assert_eq!(crate::settings::load(&db).unwrap().model, bases[0].model);
    let mut main_bot = bases[0].clone();
    main_bot.read_files = false;
    main_bot.write_files = false;
    main_bot.run_commands = false;
    main_bot.local_actions = false;
    save(&db, &main_bot).unwrap();
    let effective = runtime_settings_by_id(&db, &saved.id).unwrap();
    assert!(
        !effective.read_files
            && !effective.write_files
            && !effective.run_commands
            && !effective.local_actions
    );
    assert!(remove(&db, &saved.id, 1).is_err());
    remove(&db, &saved.id, 2).unwrap();
    assert!(load_by_id(&db, &saved.id).is_err());
    assert!(remove(&db, Role::Bot.id(), 2).is_err());
    assert!(remove(&db, Role::Helper.id(), 1).is_err());
    assert!(remove(&db, Role::Guide.id(), 1).is_err());
}

#[test]
fn new_types_preserve_provider_validation_and_disabled_instances_cannot_run() {
    let db = database();
    let mut draft = load(&db, Role::Guide).unwrap();
    draft.name = "Guía secundaria".into();
    let guide = create(&db, &draft).unwrap();
    assert_eq!(guide.role, "guide");
    assert!(runtime_settings_by_id(&db, &guide.id).is_err());
    draft.role = "Evaluador".into();
    let evaluator = create(&db, &draft).unwrap();
    assert_eq!(evaluator.provider, "openrouter_decisions");
    draft.read_files = true;
    assert!(create(&db, &draft).is_err());
    draft.read_files = false;
    draft.role = "bot".into();
    assert!(create(&db, &draft).is_err());
    let mut helper = load(&db, Role::Helper).unwrap();
    helper.enabled = false;
    let disabled = create(&db, &helper).unwrap();
    assert!(runtime_settings_by_id(&db, &disabled.id).is_err());
    helper.name = "   ".into();
    assert!(create(&db, &helper).is_err());
    helper.name = "Nuevo".into();
    helper.role = "tipo\nnuevo".into();
    assert!(create(&db, &helper).is_err());
}

#[test]
fn expansion_migration_preserves_referenced_bases_and_all_their_settings() {
    let db = Database::memory().unwrap();
    let module = LlmModule::default();
    db.migrate(module.name(), &module.migrations()[..9])
        .unwrap();
    db.with_connection(|c| {
        c.execute_batch("UPDATE llm_architects SET revision=7,personality='Configurado',instructions='Conservar',model='saved-model' WHERE id='gennie';
            INSERT INTO llm_delegations(id,request_id,bot_id,source_id,origin,job_id,plugin_id,objective) VALUES('d','r','gennie','plugin_builder','{}',1,'p','o');
            INSERT INTO llm_execution_notices(event_key,bot_id,document) VALUES('n','gennie','{}');")?;
        Ok(())
    }).unwrap();
    db.migrate(module.name(), module.migrations()).unwrap();
    let bot = load(&db, Role::Bot).unwrap();
    assert_eq!(bot.revision, 7);
    assert_eq!(bot.personality, "Configurado");
    assert_eq!(bot.instructions, "Conservar");
    assert_eq!(bot.model, "saved-model");
    assert_eq!(bot.name, "Gennie");
    db.with_connection(|c| {
        assert_eq!(
            c.query_row("SELECT count(*) FROM llm_delegations", [], |r| r
                .get::<_, i64>(0))?,
            1
        );
        assert_eq!(
            c.query_row("SELECT count(*) FROM llm_execution_notices", [], |r| r
                .get::<_, i64>(0))?,
            1
        );
        assert!(!c.prepare("PRAGMA foreign_key_check")?.exists([])?);
        Ok(())
    })
    .unwrap();
    // Segunda migración idempotente: no recrea ni restablece perfiles.
    db.migrate(module.name(), module.migrations()).unwrap();
    assert_eq!(load(&db, Role::Bot).unwrap(), bot);
}
