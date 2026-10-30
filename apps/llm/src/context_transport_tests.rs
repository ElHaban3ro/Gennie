use super::*;
use crate::context::{Context, PreparedUser, Reference, Update};
struct Journal {
    events: Mutex<Vec<&'static str>>,
    fail_confirm: bool,
    pending: bool,
}
impl ConversationJournal for Journal {
    fn activate(
        &self,
        _: &gennie_db::SqliteConnection,
        _: &str,
        _: &str,
        _: Option<&str>,
    ) -> Result<()> {
        Ok(())
    }
    fn append(&self, _: &str, _: &str, _: &str, _: &str) -> Result<()> {
        Ok(())
    }
    fn committed(&self) {}
    fn prepare_user(
        &self,
        provider: &str,
        thread: &str,
        text: &str,
        reply: Option<i64>,
    ) -> Result<PreparedUser> {
        assert_eq!(
            (provider, thread, text, reply),
            ("codex", "active", "  Entrada literal\n", None)
        );
        self.events.lock().unwrap().push("reserved");
        let context = self.pending.then(|| Context {
            updates: vec![Update {
                message_id: 7,
                text: "Aviso ficticio".into(),
                created_at: "2026-10-29T13:00:00Z".into(),
                reference: Reference::PluginBuild {
                    job_id: 1,
                    plugin_id: "fixture".into(),
                    source_sequence: 1,
                },
            }],
            ..Context::default()
        });
        Ok(PreparedUser {
            quote: None,
            context,
            batch_id: self.pending.then_some(9),
        })
    }
    fn confirm_context(&self, provider: &str, thread: &str, batch: i64) -> Result<()> {
        assert_eq!((provider, thread, batch), ("codex", "active", 9));
        self.events.lock().unwrap().push("accepted");
        if self.fail_confirm {
            Err("fixture receipt failure".into())
        } else {
            Ok(())
        }
    }
    fn uncertain_context(&self, _: &str, _: &str, _: i64) -> Result<()> {
        self.events.lock().unwrap().push("uncertain");
        Ok(())
    }
    fn begin_turn(&self, _: &str, _: &str, _: &str) -> Result<()> {
        self.events.lock().unwrap().push("turn");
        Ok(())
    }
}
fn session(journal: Arc<Journal>, response: &str) -> Session {
    let script = r#"
stage=__INITIAL__
while IFS= read -r request; do
 case "$stage:$request" in
 'inject:'*'"method":"thread/inject_items"'*)
  case "$request" in *'"role":"assistant"'*) ;; *) exit 7;; esac
  case "$request" in *'Aviso ficticio'*) ;; *) exit 7;; esac
  printf '%s\n' '__RESPONSE__'; stage=turn;;
 'turn:'*'"method":"turn/start"'*)
  case "$request" in *'"text":"  Entrada literal\n"'*) ;; *) exit 8;; esac
  printf '%s\n' '{"id":4,"result":{"turn":{"id":"turn"}}}' '{"method":"item/completed","params":{"threadId":"active","turnId":"turn","item":{"type":"agentMessage","id":"reply","text":"Respuesta ficticia"}}}' '{"method":"turn/completed","params":{"turn":{"id":"turn","status":"completed"}}}'; stage=title;;
 'title:'*'"method":"thread/read"'*) printf '%s\n' '{"id":7,"result":{"thread":{"id":"active","name":null}}}';;
 *) exit 9;;
 esac
done
"#;
    let script = script
        .replace(
            "__INITIAL__",
            if journal.pending { "inject" } else { "turn" },
        )
        .replace("__RESPONSE__", response);
    let mut child = Command::new("/bin/sh")
        .args(["-c", &script])
        .process_group(0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let process = ProcessHandle::default();
    *process.0.lock().unwrap() = Some(child);
    let (sender, lines) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if sender.send(line.unwrap()).is_err() {
                break;
            }
        }
    });
    Session {
        stdin,
        lines,
        reader: Some(reader),
        process,
        thread_id: "active".into(),
        journal: Some(journal),
        cwd: std::env::temp_dir(),
        model: "fixture".into(),
        effort: "low".into(),
        title_effort: None,
        settings: Settings::default(),
        tool_names: Vec::new(),
        has_mcp: false,
        cancellation: None,
        ticket: None,
    }
}
#[test]
fn native_injection_is_acknowledged_before_the_unchanged_user_turn() {
    for pending in [false, true] {
        let j = Arc::new(Journal {
            events: Mutex::new(Vec::new()),
            fail_confirm: false,
            pending,
        });
        let mut s = session(j.clone(), r#"{"id":14,"result":{}}"#);
        let mut messages = Vec::new();
        s.speak_stream("  Entrada literal\n", |e| messages.push(e))
            .unwrap();
        assert_eq!(
            *j.events.lock().unwrap(),
            if pending {
                vec!["reserved", "accepted", "turn"]
            } else {
                vec!["reserved", "turn"]
            }
        );
        assert_eq!(
            messages
                .iter()
                .filter(|e| matches!(e, StreamEvent::UserSaved(_)))
                .count(),
            1
        );
        assert_eq!(
            messages
                .iter()
                .filter(|e| matches!(e, StreamEvent::Completed(_)))
                .count(),
            1
        );
    }
}
#[test]
fn provider_or_receipt_failure_stops_before_a_turn_and_keeps_uncertainty() {
    for (response, fail_confirm) in [
        (
            r#"{"id":14,"error":{"code":-32601,"message":"unsupported"}}"#,
            false,
        ),
        (r#"{"id":14,"result":null}"#, false),
        (r#"{"id":14,"result":{}}"#, true),
    ] {
        let j = Arc::new(Journal {
            events: Mutex::new(Vec::new()),
            fail_confirm,
            pending: true,
        });
        let mut s = session(j.clone(), response);
        assert!(s.speak_stream("  Entrada literal\n", |_| {}).is_err());
        assert_eq!(
            *j.events.lock().unwrap(),
            if fail_confirm {
                vec!["reserved", "accepted", "uncertain"]
            } else {
                vec!["reserved", "uncertain"]
            }
        );
    }
}
