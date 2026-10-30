use super::*;
use std::{
    io::{BufRead, BufReader},
    os::unix::process::CommandExt,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
};

fn mock(script: &str) -> (Client, Arc<AtomicUsize>) {
    let mut child = Command::new("/bin/sh")
        .args(["-c", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .unwrap();
    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let process = ProcessHandle::default();
    *process.0.lock().unwrap() = Some(child);
    let (tx, lines) = mpsc::channel();
    let count = Arc::new(AtomicUsize::new(0));
    let observed = count.clone();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout)
            .lines()
            .map_while(std::result::Result::ok)
        {
            if line.contains("fixtureCalled") {
                observed.fetch_add(1, Ordering::SeqCst);
            }
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let session = Session {
        stdin,
        lines,
        reader: Some(reader),
        process,
        thread_id: "private-mcp".into(),
        journal: None,
        cwd: PathBuf::from("/tmp"),
        model: String::new(),
        effort: String::new(),
        title_effort: None,
        settings: Settings::default(),
        tool_names: Vec::new(),
        has_mcp: false,
        cancellation: None,
        ticket: None,
    };
    (
        Client {
            session,
            next_id: 10,
            timeout: Duration::from_millis(150),
            usable: true,
        },
        count,
    )
}

#[test]
#[ignore = "Requiere Codex instalado; HTTP loopback ficticio, sin cuenta ni inferencia"]
fn installed_codex_calls_streamable_http_mcp_without_model() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::atomic::AtomicBool,
    };
    let mut registry = gennie_mcp::Registry::default();
    registry.register([gennie_mcp::Tool{
        definition:json!({"name":"http_echo","description":"Consulta HTTP ficticia","inputSchema":{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}}),
        call:Box::new(|args|Ok(json!({"echo":args["text"]}))),
    }]).unwrap();
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    eprintln!("Servidor MCP HTTP ficticio: {address}");
    let finished = Arc::new(AtomicBool::new(false));
    let flag = finished.clone();
    struct HttpFixture {
        finished: Arc<AtomicBool>,
        worker: Option<std::thread::JoinHandle<()>>,
    }
    impl Drop for HttpFixture {
        fn drop(&mut self) {
            self.finished.store(true, Ordering::Release);
            if let Some(worker) = self.worker.take() {
                worker.join().unwrap();
            }
        }
    }
    let worker = std::thread::spawn(move || {
        while !flag.load(Ordering::Acquire) {
            let (mut stream, _) = match listener.accept() {
                Ok(value) => value,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(_) => break,
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() {
                continue;
            }
            let post = line.starts_with("POST ");
            let mut length = 0;
            loop {
                line.clear();
                if reader.read_line(&mut line).is_err() || line == "\r\n" || line.is_empty() {
                    break;
                }
                if let Some((name, value)) = line.split_once(':')
                    && name.eq_ignore_ascii_case("content-length")
                {
                    length = value.trim().parse::<usize>().unwrap();
                }
            }
            if !post {
                let _=stream.write_all(b"HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                continue;
            }
            if length > 128 * 1024 {
                break;
            }
            let mut body = vec![0; length];
            if reader.read_exact(&mut body).is_err() {
                continue;
            }
            let request = serde_json::from_slice(&body).unwrap();
            let response = registry.respond(request);
            let (status, body) = match response {
                Some(value) => ("200 OK", value.to_string()),
                None => ("202 Accepted", String::new()),
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    let _http = HttpFixture {
        finished,
        worker: Some(worker),
    };
    let base = std::env::temp_dir().join(format!(
        "gennie-mcp-http-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    struct Directory(PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _directory = Directory(base.clone());
    let mut client = Client::connect(
        base.clone(),
        &Transport::Http {
            url: format!("http://{address}/mcp"),
            bearer_token_env_var: None,
        },
        10,
        Control::default(),
    )
    .unwrap();
    assert!(
        client.inventory().unwrap()["tools"]
            .get("http_echo")
            .is_some()
    );
    assert_eq!(
        client.call("http_echo", json!({"text":"HTTP-27"})).unwrap()["structuredContent"]["echo"],
        "HTTP-27"
    );
    assert!(!base.join("cli-home/auth.json").exists());
}

#[test]
fn endpoint_config_is_data_and_contains_no_account_or_inference_options() {
    let transport = Transport::Stdio {
        command: "/tmp/fixture \"quoted\"".into(),
        args: vec!["a\nb".into(), "$(never-evaluate)".into()],
        cwd: Some("/tmp/space dir".into()),
        env_vars: vec!["FIXTURE_TOKEN".into()],
    };
    let config = transport.config(60).unwrap();
    assert!(config.contains(r#"command="/tmp/fixture \"quoted\"""#));
    assert!(config.contains(r#"args=["a\nb","$(never-evaluate)"]"#));
    assert!(!config.contains("auth.json"));
    assert!(transport.config(0).is_err());
    let http = Transport::Http {
        url: "https://example.com/mcp".into(),
        bearer_token_env_var: Some("FIXTURE_TOKEN".into()),
    }
    .config(60)
    .unwrap();
    assert!(http.contains(r#"bearer_token_env_var="FIXTURE_TOKEN""#));
}

#[test]
fn inventory_follows_pages_and_direct_call_does_not_generate_a_turn() {
    let (mut client, _) = mock(
        r#"
while IFS= read -r request; do
 case "$request" in
 *'"id":11,'*) printf '%s\n' '{"id":11,"result":{"data":[{"name":"external","authStatus":"unsupported","tools":{"first":{"name":"first","inputSchema":{"type":"object"}}}}],"nextCursor":"page-2"}}';;
 *'"id":12,'*) printf '%s\n' '{"id":12,"result":{"data":[{"name":"external","tools":{"second":{"name":"second","inputSchema":{"type":"object"}}}}],"nextCursor":null}}';;
 *'"method":"mcpServer/tool/call"'*) printf '%s\n' '{"id":13,"result":{"content":[{"type":"text","text":"fixture"}],"structuredContent":{"ok":true},"isError":false}}';;
 *) exit 8;;
 esac
done
"#,
    );
    let inventory = client.inventory().unwrap();
    assert_eq!(inventory["tools"].as_object().unwrap().len(), 2);
    assert_eq!(
        client.call("first", json!({})).unwrap()["structuredContent"]["ok"],
        true
    );
}

#[test]
fn repeated_cursor_and_duplicate_identities_are_not_silent_truncation() {
    for tool in ["first", "second"] {
        let script = r#"
while IFS= read -r request; do
 case "$request" in
 *'"id":11,'*) printf '%s\n' '{"id":11,"result":{"data":[{"name":"external","tools":{"first":{"name":"first"}}}],"nextCursor":"repeat"}}';;
 *) printf '%s\n' '{"id":12,"result":{"data":[{"name":"external","tools":{"__TOOL__":{"name":"__TOOL__"}}}],"nextCursor":"repeat"}}';;
 esac
done
"#.replace("__TOOL__",tool);
        let (mut client, _) = mock(&script);
        assert!(client.inventory().is_err());
    }
}

#[test]
fn advancing_empty_pages_cannot_keep_discovery_running_without_a_bound() {
    let (mut client, _) = mock(
        r#"
i=10
while IFS= read -r request; do
 i=$((i+1))
 printf '{"id":%s,"result":{"data":[{"name":"external","tools":{}}],"nextCursor":"page-%s"}}\n' "$i" "$i"
done
"#,
    );
    assert!(
        client
            .inventory()
            .unwrap_err()
            .to_string()
            .contains("páginas")
    );
}

#[test]
fn a_lost_call_closes_its_own_process_without_repeating_the_effect() {
    let (mut client, count) = mock(
        r#"
while IFS= read -r request; do
 case "$request" in
 *'"method":"mcpServer/tool/call"'*) printf '%s\n' '{"method":"fixtureCalled","params":{}}'; exit 0;;
 *) exit 8;;
 esac
done
"#,
    );
    assert!(client.call("write", json!({"value":"fixture"})).is_err());
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert!(client.session.process.0.lock().unwrap().is_none());
    assert!(client.call("write", json!({"value":"fixture"})).is_err());
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn cancelled_context_reaps_only_its_client_and_never_repeats_a_sent_call() {
    let (mut client, count) = mock(
        r#"
while IFS= read -r request; do
 printf '%s\n' '{"method":"fixtureCalled","params":{}}'
done
"#,
    );
    client.timeout = Duration::from_secs(10);
    let context = gennie_mcp::CallContext::default();
    client.bind_context(Some(context.clone()));
    let observed = count.clone();
    let task = std::thread::spawn(move || {
        let result = client.call("write", json!({"value":"fixture"}));
        (client, result)
    });
    let deadline = Instant::now() + Duration::from_secs(2);
    while observed.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    context.cancel();
    let started = Instant::now();
    let (mut client, result) = task.join().unwrap();
    assert!(result.unwrap_err().is::<gennie_mcp::Cancelled>());
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert!(!client.is_usable());
    assert!(client.session.process.0.lock().unwrap().is_none());
    client.bind_context(None);
    assert!(client.call("write", json!({})).is_err());
    assert_eq!(count.load(Ordering::SeqCst), 1);
    let (mut other, _) = mock(
        r#"
while IFS= read -r request; do
 printf '%s\n' '{"id":11,"result":{"content":[{"type":"text","text":"independent"}]}}'
done
"#,
    );
    assert_eq!(
        other.call("read", json!({})).unwrap()["content"][0]["text"],
        "independent"
    );
}

#[test]
#[ignore = "Requiere Codex/helper instalados; MCP ficticio, sin cuenta ni inferencia"]
fn installed_codex_calls_mcp_without_account_model_or_conversation() {
    let mut registry = gennie_mcp::Registry::default();
    let count = Arc::new(AtomicUsize::new(0));
    let effects = count.clone();
    registry.register([gennie_mcp::Tool {
        definition:json!({"name":"fixture_echo","description":"Consulta ficticia local","inputSchema":{"type":"object","properties":{"text":{"type":"string"}},"required":["text"],"additionalProperties":false},"annotations":{"readOnlyHint":true}}),
        call:Box::new(move |args| { effects.fetch_add(1,Ordering::SeqCst); Ok(json!({"echo":args["text"]})) }),
    }]).unwrap();
    let helper = std::env::var_os("GENNIE_TEST_HELPER")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/flutter/debug/gennie")
        });
    let mut server = gennie_mcp::Server::start(registry, &helper).unwrap();
    let base = std::env::temp_dir().join(format!(
        "gennie-mcp-client-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    struct Directory(PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _directory = Directory(base.clone());
    let launch = server.launch();
    let endpoint = Transport::Stdio {
        command: launch.helper.to_string_lossy().into(),
        args: vec!["--mcp".into(), launch.socket.to_string_lossy().into()],
        cwd: None,
        env_vars: vec![],
    };
    let result = (|| -> Result<()> {
        let mut client = Client::connect(base.clone(), &endpoint, 10, Control::default())?;
        let inventory = client.inventory()?;
        assert!(inventory["tools"].get("fixture_echo").is_some());
        assert!(!base.join("cli-home/auth.json").exists());
        let output = client.call("fixture_echo", json!({"text":"ECHO-27"}))?;
        assert_eq!(output["structuredContent"]["echo"], "ECHO-27");
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert!(!base.join("cli-home/sessions").exists());
        Ok(())
    })();
    server.stop().unwrap();
    result.unwrap();
}
