//! Integración local de SOC con Recordatorios mediante EventKit.
//! Las operaciones son bloqueantes: los consumidores deben llamarlas fuera del hilo de Slint.

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use soc_core::{AppModule, AppState, Result};
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver},
    },
    thread::JoinHandle,
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Reminder {
    pub id: String,
    pub list_id: String,
    pub title: String,
    pub notes: Option<String>,
    pub due_at: Option<String>,
    pub completed: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReminderList {
    pub id: String,
    pub title: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewReminder {
    pub list_id: Option<String>,
    pub title: String,
    pub notes: Option<String>,
    pub due_at: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderUpdate {
    pub title: Option<String>,
    pub notes: Option<String>,
    pub due_at: Option<String>,
    pub completed: Option<bool>,
    pub clear_notes: bool,
    pub clear_due_at: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChangeKind {
    Created,
    Updated,
    Deleted,
    Ready,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ReminderChange {
    pub kind: ChangeKind,
    pub id: String,
}

#[derive(Deserialize)]
struct Reply<T> {
    result: Option<T>,
    error: Option<String>,
}

struct Watcher {
    child: Arc<Mutex<Child>>,
    handle: JoinHandle<()>,
}

#[derive(Default)]
pub struct RemindersPluginModule {
    watcher: Mutex<Option<Watcher>>,
    events: Arc<Mutex<Option<Receiver<ReminderChange>>>>,
}

#[derive(Clone)]
pub struct Controller {
    bridge: PathBuf,
    events: Arc<Mutex<Option<Receiver<ReminderChange>>>>,
}

impl RemindersPluginModule {
    pub fn controller(&self) -> Result<Controller> {
        if self
            .watcher
            .lock()
            .map_err(|_| "Watcher inválido")?
            .is_none()
        {
            return Err("Reminders plugin no está activo".into());
        }
        Ok(Controller {
            bridge: bridge_path()?,
            events: self.events.clone(),
        })
    }
}

impl AppModule for RemindersPluginModule {
    fn name(&self) -> &'static str {
        "reminders_plugin"
    }

    fn active_by_default(&self) -> bool {
        cfg!(target_os = "macos")
    }

    fn start(&self, _: &AppState) -> Result<()> {
        let mut watcher = self.watcher.lock().map_err(|_| "Watcher inválido")?;
        if watcher.is_some() {
            return Ok(());
        }
        let bridge = bridge_path()?;
        let mut child = Command::new(bridge)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        child
            .stdin
            .take()
            .ok_or("Sin entrada del puente")?
            .write_all(b"{\"action\":\"watch\"}\n")?;
        let stdout = child.stdout.take().ok_or("Sin salida del puente")?;
        let child = Arc::new(Mutex::new(child));
        let (sender, receiver) = mpsc::channel();
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let handle = std::thread::Builder::new()
            .name("reminders-events".into())
            .spawn(move || {
                let mut lines = BufReader::new(stdout).lines();
                let ready = match lines.next() {
                    Some(Ok(line)) => match serde_json::from_str::<Reply<ReminderChange>>(&line) {
                        Ok(Reply {
                            result: Some(change),
                            ..
                        }) if change.kind == ChangeKind::Ready => Ok(()),
                        Ok(Reply {
                            error: Some(error), ..
                        }) => Err(error),
                        _ => Err("El observador EventKit no confirmó el arranque".into()),
                    },
                    _ => Err("El observador EventKit terminó antes de arrancar".into()),
                };
                if ready_sender.send(ready).is_err() {
                    return;
                }
                for line in lines {
                    let Ok(line) = line else { break };
                    match serde_json::from_str::<Reply<ReminderChange>>(&line) {
                        Ok(Reply {
                            result: Some(change),
                            ..
                        }) if change.kind != ChangeKind::Ready => {
                            let sent = sender.send(change);
                            if sent.is_err() {
                                break;
                            }
                        }
                        Ok(Reply { error: Some(_), .. }) => break,
                        _ => {}
                    }
                }
            })?;
        match ready_receiver.recv_timeout(std::time::Duration::from_secs(35)) {
            Ok(Ok(())) => {}
            other => {
                let mut process = child.lock().map_err(|_| "Proceso inválido")?;
                let _ = process.kill();
                let _ = process.wait();
                drop(process);
                let _ = handle.join();
                return Err(format!(
                    "No se pudo iniciar el observador de Recordatorios: {other:?}"
                )
                .into());
            }
        }
        *self.events.lock().map_err(|_| "Eventos inválidos")? = Some(receiver);
        *watcher = Some(Watcher { child, handle });
        Ok(())
    }

    fn stop(&self, _: &AppState) -> Result<()> {
        if let Some(watcher) = self.watcher.lock().map_err(|_| "Watcher inválido")?.take() {
            let mut child = watcher.child.lock().map_err(|_| "Proceso inválido")?;
            let _ = child.kill();
            let _ = child.wait();
            drop(child);
            watcher
                .handle
                .join()
                .map_err(|_| "Watcher terminó incorrectamente")?;
        }
        *self.events.lock().map_err(|_| "Eventos inválidos")? = None;
        Ok(())
    }
}

impl Controller {
    pub fn lists(&self) -> Result<Vec<ReminderList>> {
        self.call(serde_json::json!({ "action": "lists" }))
    }
    pub fn list(&self, list_id: Option<&str>) -> Result<Vec<Reminder>> {
        self.call(serde_json::json!({ "action": "list", "listId": list_id }))
    }
    pub fn get(&self, id: &str) -> Result<Reminder> {
        self.call(serde_json::json!({ "action": "get", "id": id }))
    }
    pub fn create(&self, reminder: NewReminder) -> Result<Reminder> {
        if reminder.title.trim().is_empty() {
            return Err("El título del recordatorio es obligatorio".into());
        }
        let mut request = serde_json::to_value(reminder)?;
        request["action"] = "create".into();
        self.call(request)
    }
    pub fn update(&self, id: &str, update: ReminderUpdate) -> Result<Reminder> {
        if update
            .title
            .as_deref()
            .is_some_and(|title| title.trim().is_empty())
        {
            return Err("El título del recordatorio es obligatorio".into());
        }
        let mut request = serde_json::to_value(update)?;
        request["action"] = "update".into();
        request["id"] = id.into();
        self.call(request)
    }
    pub fn delete(&self, id: &str) -> Result<()> {
        let _: String = self.call(serde_json::json!({ "action": "delete", "id": id }))?;
        Ok(())
    }
    pub fn drain_changes(&self) -> Result<Vec<ReminderChange>> {
        let events = self.events.lock().map_err(|_| "Eventos inválidos")?;
        let receiver = events
            .as_ref()
            .ok_or("Observador de Recordatorios detenido")?;
        let mut changes = Vec::new();
        loop {
            match receiver.try_recv() {
                Ok(change) => changes.push(change),
                Err(mpsc::TryRecvError::Empty) => return Ok(changes),
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err("Observador de Recordatorios desconectado".into());
                }
            }
        }
    }
    fn call<T: DeserializeOwned>(&self, request: serde_json::Value) -> Result<T> {
        let mut child = Command::new(&self.bridge)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let mut input = serde_json::to_vec(&request)?;
        input.push(b'\n');
        child
            .stdin
            .take()
            .ok_or("Sin entrada del puente")?
            .write_all(&input)?;
        let output = child.wait_with_output()?;
        let reply: Reply<T> = serde_json::from_slice(&output.stdout)?;
        if let Some(error) = reply.error {
            return Err(error.into());
        }
        reply
            .result
            .ok_or_else(|| "El puente no devolvió datos".into())
    }
}

fn bridge_path() -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        use std::{fs, os::unix::fs::PermissionsExt};
        let bytes = include_bytes!(env!("SOC_REMINDERS_BRIDGE"));
        let home = std::env::var_os("HOME").ok_or("Falta HOME para el puente EventKit")?;
        let directory =
            PathBuf::from(home).join("Library/Application Support/soc/reminders-plugin");
        fs::create_dir_all(&directory)?;
        let path = directory.join("soc-reminders-bridge");
        if fs::read(&path).is_ok_and(|existing| existing == bytes) {
            return Ok(path);
        }
        let temporary = directory.join(format!(
            ".soc-reminders-bridge-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        fs::write(&temporary, bytes)?;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o700))?;
        if let Err(error) = fs::rename(&temporary, &path) {
            let _ = fs::remove_file(&temporary);
            return Err(error.into());
        }
        Ok(path)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("EventKit solo está disponible en macOS".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_fields_match_native_protocol() {
        let request = serde_json::to_value(NewReminder {
            list_id: Some("list".into()),
            title: "Tarea".into(),
            notes: None,
            due_at: None,
        })
        .unwrap();
        assert_eq!(request["listId"], "list");
        assert_eq!(request["title"], "Tarea");
        let update = serde_json::to_value(ReminderUpdate {
            clear_notes: true,
            completed: Some(true),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(update["clearNotes"], true);
        assert_eq!(update["completed"], true);
    }

    #[test]
    fn change_decodes_without_reminder_content() {
        let reply: Reply<ReminderChange> =
            serde_json::from_str(r#"{"result":{"kind":"created","id":"abc"},"error":null}"#)
                .unwrap();
        assert_eq!(
            reply.result.unwrap(),
            ReminderChange {
                kind: ChangeKind::Created,
                id: "abc".into()
            }
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "modifica temporalmente Recordatorios reales; ejecutar solo con autorización"]
    fn native_controller_roundtrip() {
        let controller = Controller {
            bridge: bridge_path().unwrap(),
            events: Arc::default(),
        };
        let title = format!(
            "SOC prueba temporal {}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let created = controller
            .create(NewReminder {
                list_id: None,
                title: title.clone(),
                notes: Some("Temporal".into()),
                due_at: None,
            })
            .unwrap();
        let result = (|| -> Result<()> {
            assert_eq!(controller.get(&created.id)?.title, title);
            assert!(
                controller
                    .list(Some(&created.list_id))?
                    .iter()
                    .any(|r| r.id == created.id)
            );
            let updated = controller.update(
                &created.id,
                ReminderUpdate {
                    completed: Some(true),
                    clear_notes: true,
                    ..Default::default()
                },
            )?;
            assert!(updated.completed);
            assert!(updated.notes.is_none());
            Ok(())
        })();
        let cleanup = controller.delete(&created.id);
        result.unwrap();
        cleanup.unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requiere acceso real a Recordatorios en macOS"]
    fn native_watcher_lifecycle() {
        let state = AppState {
            config: Arc::new(soc_config::Config {
                database_path: "unused".into(),
                db_busy_timeout_ms: 50,
            }),
            db: soc_core::Database::memory().unwrap(),
            shutdown: soc_core::Shutdown::default(),
        };
        let module = RemindersPluginModule::default();
        module.start(&state).unwrap();
        module.start(&state).unwrap();
        let controller = module.controller().unwrap();
        assert!(!controller.lists().unwrap().is_empty());
        assert!(controller.drain_changes().unwrap().is_empty());
        module.stop(&state).unwrap();
        module.stop(&state).unwrap();
        assert!(module.controller().is_err());
    }
}
