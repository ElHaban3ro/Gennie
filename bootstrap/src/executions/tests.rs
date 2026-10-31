use super::*;
use gennie_assignments::{
    AssignmentsModule, Store,
    execution::{Configure, Misfire, Runner, Target},
    proposals::Reference,
    schedule::{DstPolicy, MissingTime, RepeatedTime, Trigger, utc},
};
use gennie_core::{AppModule, AppState, Shutdown};
use gennie_db::Database;
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
struct NoBuild;
impl gennie_plugins::BuildJournal for NoBuild {
    fn origin(&self) -> Result<i64> {
        Err("No hay construcción en esta prueba".into())
    }
    fn exists(&self, _: i64) -> Result<bool> {
        Ok(false)
    }
    fn target(&self, _: i64) -> Result<gennie_llm::delegation::Origin> {
        Err("Sin origen".into())
    }
    fn deliver(&self, _: &gennie_llm::delegation::Update) -> Result<()> {
        Err("Sin avisos de construcción".into())
    }
}
struct Harness {
    root: PathBuf,
    runner: Option<Runner>,
    module: gennie_plugins::PluginsModule,
    state: AppState,
}
impl Drop for Harness {
    fn drop(&mut self) {
        if let Some(r) = &self.runner {
            r.request_stop();
        }
        let _ = self.module.stop(&self.state);
        if let Some(mut r) = self.runner.take() {
            let _ = r.join();
        }
        let _ = self.state.db.close();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
#[test]
#[ignore = "Usa el CLI Codex instalado, MCP stdio y contador propios; sin autenticación ni inferencia"]
fn installed_scheduler_mcp_executes_once_and_does_not_repeat_a_lost_response() {
    let root = std::env::temp_dir().join(format!(
        "gennie-scheduled-mcp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let script = root.join("server.py");
    let counter = root.join("counter.txt");
    std::fs::write(&counter, "0").unwrap();
    std::fs::write(&script,r#"import sys,json,os
from pathlib import Path
counter=Path(sys.argv[1])
for line in sys.stdin:
 r=json.loads(line);i=r.get('id');m=r.get('method')
 if i is None:continue
 if m=='initialize':v={'protocolVersion':'2025-06-18','capabilities':{'tools':{}},'serverInfo':{'name':'own-counter','version':'1'}}
 elif m=='tools/list':v={'tools':[{'name':'increment','description':'Incrementa solo el contador ficticio de esta prueba','inputSchema':{'type':'object','properties':{'lose_response':{'type':'boolean'}},'additionalProperties':False}}]}
 elif m=='tools/call':
  count=int(counter.read_text())+1;counter.write_text(str(count))
  if r['params']['arguments'].get('lose_response'):os._exit(0)
  v={'content':[{'type':'text','text':'Contador actualizado'}],'structuredContent':{'count':count},'isError':False}
 else:v={}
 print(json.dumps({'jsonrpc':'2.0','id':i,'result':v}),flush=True)
"#).unwrap();
    let db = Database::open(&root.join("fixture.sqlite3"), 1000).unwrap();
    db.migrate("events", AssignmentsModule.migrations())
        .unwrap();
    let browser = gennie_browser::BrowserModule::default();
    let helper = std::env::current_exe().unwrap();
    let module = gennie_plugins::PluginsModule::new(
        db.clone(),
        browser.controller(),
        Arc::new(NoBuild),
        gennie_plugins::AgentSettings::shared(Arc::new(|| Ok(gennie_llm::Settings::default()))),
        helper,
        root.join("builds"),
    );
    db.migrate(module.migration_namespace(), module.migrations())
        .unwrap();
    let state = AppState {
        db: db.clone(),
        config: Arc::new(gennie_config::Config {
            database_path: root.join("fixture.sqlite3"),
            db_busy_timeout_ms: 1000,
        }),
        shutdown: Shutdown::default(),
    };
    module.start(&state).unwrap();
    let plugins = module.controller();
    let store = Store::new(db.clone());
    let runner = Runner::start(
        store.clone(),
        Arc::new(Host {
            guide: None,
            plugins: plugins.clone(),
            execution_available: true,
            bot: None,
            db: db.clone(),
        }),
        &root.join("scheduler.lock"),
    )
    .unwrap();
    let mut harness = Harness {
        root: root.clone(),
        runner: Some(runner),
        module,
        state,
    };
    let controller = harness.runner.as_ref().unwrap().controller();
    let saved=plugins.request(json!({"action":"mcp_save","expected_revision":0,"specification":{"id":"own_counter","name":"Contador ficticio","connection":{"type":"stdio","command":"python3","args":["-u",script,counter],"env_vars":[]},"timeout_seconds":10}})).unwrap();
    let connected=plugins.request(json!({"action":"mcp_connect","server_id":"own_counter","expected_revision":saved["server"]["revision"]})).unwrap();
    let ready=plugins.request(json!({"action":"mcp_permissions","server_id":"own_counter","expected_revision":connected["server"]["revision"],"allowed_tools":["increment"],"enabled":true})).unwrap();
    let revision = ready["server"]["revision"].as_i64().unwrap();
    for (index, lose) in [false, true].into_iter().enumerate() {
        let now = i64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        )
        .unwrap();
        let document=serde_json::from_value(json!({"title":"Contador de prueba","description":"Modificar solo el contador temporal autorizado","kind":"one_time","priority":"normal","status":"active","timezone":"Etc/UTC","notify_at":utc(now+3).unwrap(),"method":{"plugin_id":"mcp:own_counter","functions":["increment"]},"authorization":{"scope":"Incrementar una vez el contador ficticio propio","allowed_routes":["mcp"]}})).unwrap();
        let row = store.create(&format!("fixture-{index}"), document).unwrap();
        let request = Configure {
            contextual_notice: false,
            delivery: None,
            assignment: Reference {
                id: row.id,
                instance_id: row.instance_id,
                revision: row.revision,
            },
            expected_policy_revision: 0,
            trigger: Trigger::Delivery,
            dst: DstPolicy {
                repeated: RepeatedTime::Earlier,
                missing: MissingTime::Skip,
            },
            target: Target::Mcp {
                server_id: "own_counter".into(),
                tool: "increment".into(),
                revision,
            },
            arguments: json!({"lose_response":lose}),
            misfire: Misfire::RunLatest,
            max_lateness_minutes: 1,
            until: None,
            enabled: true,
        };
        let policy = controller
            .request(json!({"action":"configure","configuration":request}))
            .unwrap();
        let id = policy["id"].as_str().unwrap();
        let deadline = Instant::now() + Duration::from_secs(12);
        let mut terminal = false;
        while Instant::now() < deadline {
            let runs = controller
                .request(json!({"action":"runs","policy_id":id}))
                .unwrap();
            if let Some(run) = runs["runs"].as_array().unwrap().first() {
                let state = run["state"].as_str().unwrap();
                if matches!(state, "succeeded" | "uncertain") {
                    assert_eq!(state, if lose { "uncertain" } else { "succeeded" });
                    terminal = true;
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(30));
        }
        assert!(
            terminal,
            "La ejecución no terminó dentro del plazo de la sonda"
        );
        assert_eq!(
            std::fs::read_to_string(&counter).unwrap(),
            (index + 1).to_string()
        );
        if lose {
            let result = controller
                .request(json!({"action":"list","assignment_id":row.id}))
                .unwrap();
            assert_eq!(result["policies"][0]["state"], "review_required");
            assert_eq!(result["policies"][0]["enabled"], false);
        }
    }
    harness.runner.as_ref().unwrap().request_stop();
    harness.module.stop(&harness.state).unwrap();
    harness.runner.as_mut().unwrap().join().unwrap();
    store.recover_executions().unwrap();
    assert_eq!(std::fs::read_to_string(&counter).unwrap(), "2");
    db.with_connection(|c| {
        assert_eq!(
            c.query_row("SELECT count(*) FROM assignment_execution_runs", [], |r| {
                r.get::<_, i64>(0)
            })?,
            2
        );
        assert_eq!(
            c.query_row("SELECT count(*) FROM plugin_build_jobs", [], |r| r
                .get::<_, i64>(0))?,
            0
        );
        Ok(())
    })
    .unwrap();
    eprintln!("scheduler_mcp_effects=2 successful=1 uncertain=1 automatic_replays=0");
}
