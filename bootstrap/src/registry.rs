use gennie_core::{AppModule, Result};
use gennie_presence::PresenceModule;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

pub fn build(
    presence: Arc<PresenceModule>,
    transcription: Arc<gennie_transcription::TranscriptionModule>,
    decisions: Arc<gennie_decisions::DecisionsModule>,
    llm: Arc<gennie_llm::LlmModule>,
    tts: Arc<gennie_tts::TtsModule>,
    reminders: Arc<gennie_reminders_plugin::RemindersPluginModule>,
    browser: Arc<gennie_browser::BrowserModule>,
) -> Vec<Arc<dyn AppModule>> {
    vec![
        transcription,
        decisions,
        llm,
        tts,
        reminders,
        presence,
        Arc::new(gennie_assignments::AssignmentsModule),
        browser,
    ]
}

pub fn order(modules: &[Arc<dyn AppModule>]) -> Result<Vec<usize>> {
    let mut names = BTreeMap::new();
    let mut migration_namespaces = BTreeSet::new();
    for (i, module) in modules.iter().enumerate() {
        let name = module.name();
        if name.is_empty()
            || !name
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
        {
            return Err(format!("Nombre de módulo inválido: {name}").into());
        }
        let namespace = module.migration_namespace();
        if namespace.is_empty()
            || !namespace
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
            || !migration_namespaces.insert(namespace)
        {
            return Err("Identidad de migraciones inválida o repetida".into());
        }
        if names.insert(name, i).is_some() {
            return Err(format!("Módulo duplicado: {name}").into());
        }
    }
    fn visit(
        i: usize,
        modules: &[Arc<dyn AppModule>],
        names: &BTreeMap<&str, usize>,
        visiting: &mut BTreeSet<usize>,
        done: &mut BTreeSet<usize>,
        output: &mut Vec<usize>,
    ) -> Result<()> {
        if done.contains(&i) {
            return Ok(());
        }
        if !visiting.insert(i) {
            return Err("Ciclo de módulos".into());
        }
        for name in modules[i].depends_on() {
            let dependency = names
                .get(name)
                .ok_or_else(|| format!("Dependencia compilada ausente: {name}"))?;
            visit(*dependency, modules, names, visiting, done, output)?;
        }
        for name in modules[i].optional_dependencies() {
            if let Some(dependency) = names.get(name) {
                visit(*dependency, modules, names, visiting, done, output)?;
            }
        }
        visiting.remove(&i);
        done.insert(i);
        output.push(i);
        Ok(())
    }
    let mut output = Vec::new();
    let mut visiting = BTreeSet::new();
    let mut done = BTreeSet::new();
    for i in 0..modules.len() {
        visit(i, modules, &names, &mut visiting, &mut done, &mut output)?;
    }
    Ok(output)
}

pub fn validate_active(modules: &[Arc<dyn AppModule>], active: &BTreeSet<String>) -> Result<()> {
    for module in modules.iter().filter(|m| active.contains(m.name())) {
        for dependency in module.depends_on() {
            if !active.contains(*dependency) {
                return Err(format!(
                    "Dependencia apagada: {} requiere {dependency}",
                    module.name()
                )
                .into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contextual_guide_starts_before_the_bot_without_becoming_required() {
        let modules: Vec<Arc<dyn AppModule>> = vec![
            Arc::new(gennie_llm::LlmModule::default()),
            Arc::new(gennie_proactivity::ProactivityModule::new(None)),
        ];
        assert_eq!(order(&modules).unwrap(), vec![1, 0]);
        assert!(validate_active(&modules, &BTreeSet::from(["llm".into()])).is_ok());
    }
    use gennie_core::AppState;
    struct Module(
        &'static str,
        &'static [&'static str],
        &'static [&'static str],
    );
    impl AppModule for Module {
        fn name(&self) -> &'static str {
            self.0
        }
        fn depends_on(&self) -> &'static [&'static str] {
            self.1
        }
        fn optional_dependencies(&self) -> &'static [&'static str] {
            self.2
        }
        fn start(&self, _: &AppState) -> Result<()> {
            Ok(())
        }
        fn stop(&self, _: &AppState) -> Result<()> {
            Ok(())
        }
    }
    fn m(
        name: &'static str,
        required: &'static [&'static str],
        optional: &'static [&'static str],
    ) -> Arc<dyn AppModule> {
        Arc::new(Module(name, required, optional))
    }
    #[test]
    fn rejects_overlapping_migration_namespaces() {
        struct Alias(&'static str, &'static str);
        impl AppModule for Alias {
            fn name(&self) -> &'static str {
                self.0
            }
            fn migration_namespace(&self) -> &'static str {
                self.1
            }
            fn start(&self, _: &AppState) -> Result<()> {
                Ok(())
            }
            fn stop(&self, _: &AppState) -> Result<()> {
                Ok(())
            }
        }
        let modules: Vec<Arc<dyn AppModule>> = vec![
            Arc::new(Alias("first", "legacy")),
            Arc::new(Alias("second", "legacy")),
        ];
        assert!(order(&modules).is_err());
        assert!(order(&[Arc::new(Alias("valid", "../bad"))]).is_err());
    }

    #[test]
    fn rejects_duplicates_cycles_and_missing() {
        assert!(order(&[m("a", &[], &[]), m("a", &[], &[])]).is_err());
        assert!(order(&[m("a", &["b"], &[])]).is_err());
        assert!(order(&[m("a", &["b"], &[]), m("b", &["a"], &[])]).is_err());
    }
    #[test]
    fn orders_optional_present_and_checks_active() {
        let modules = vec![
            m("a", &["b"], &["c", "absent"]),
            m("b", &[], &[]),
            m("c", &[], &[]),
        ];
        assert_eq!(order(&modules).unwrap(), vec![1, 2, 0]);
        assert!(validate_active(&modules, &BTreeSet::from(["a".into()])).is_err());
        validate_active(&modules, &BTreeSet::from(["a".into(), "b".into()])).unwrap();
    }
}
