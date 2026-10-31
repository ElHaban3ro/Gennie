//! Inventario del host real, sin cuenta, inferencia ni transporte externo.
use super::*;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    time::Duration,
};

fn rpc(socket: &std::path::Path, request: Value) -> Value {
    let mut stream = UnixStream::connect(socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    serde_json::to_writer(&mut stream, &request).unwrap();
    stream.write_all(b"\n").unwrap();
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).unwrap();
    serde_json::from_str(&line).unwrap()
}

#[test]
fn bot_mcp_inventory_is_independent_of_browser_and_assignments() {
    for (llm, plugins) in [(true, true), (true, false), (false, true), (false, false)] {
        let fixture = Fixture::new();
        fixture.seed_selection();
        let db = Database::open(&fixture.config.database_path, 500).unwrap();
        db.with_connection(|c| {
            c.execute("UPDATE modules SET active=?1 WHERE name='llm'", [llm])?;
            c.execute(
                "UPDATE modules SET active=?1 WHERE name='plugins'",
                [plugins],
            )?;
            Ok(())
        })
        .unwrap();
        db.close().unwrap();
        let mut backend = fixture.open();
        let launch = backend.mcp.as_ref().unwrap().launch();
        let list = rpc(
            &launch.socket,
            json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
        );
        let names = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_owned())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(names.contains("plugin_mcp_catalog"), llm && plugins);
        assert_eq!(names.contains("plugin_mcp_run"), llm && plugins);
        assert!(!names.contains("plugin_build_start"));
        assert!(
            !names
                .iter()
                .any(|n| n.starts_with("browser_") || n.starts_with("assignments_"))
        );
        if llm && plugins {
            let specification = json!({"id":"inventory_fixture","name":"Fuente sintética","connection":{"type":"http","url":"https://fixture.invalid/mcp"},"timeout_seconds":1});
            backend.plugins.request(json!({"action":"mcp_save","expected_revision":0,"specification":specification})).unwrap();
            let definitions = json!({"echo":{"name":"echo","inputSchema":{"type":"object","properties":{"text":{"type":"string"}},"required":["text"],"additionalProperties":false}}});
            // Catálogo guardado ficticio: la prueba no descubre ni conecta esa URL.
            backend.state.db.with_connection(|c| {
                c.execute("UPDATE plugin_mcp_sources SET enabled=1,tools=?1,allowed_tools='[\"echo\"]' WHERE id='inventory_fixture'",[definitions.to_string()])?;
                Ok(())
            }).unwrap();
            let catalogue = rpc(
                &launch.socket,
                json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"plugin_mcp_catalog","arguments":{"action":"get","server_id":"inventory_fixture"}}}),
            );
            assert_eq!(catalogue["result"]["isError"], false);
            assert_eq!(
                catalogue["result"]["structuredContent"]["server"]["tools"]["echo"]["name"],
                "echo"
            );
            let rejected = rpc(
                &launch.socket,
                json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"plugin_mcp_run","arguments":{"server_id":"inventory_fixture","tool":"echo","expected_revision":1,"arguments":{}}}}),
            );
            assert_eq!(
                rejected["result"]["structuredContent"]["status"],
                "invalid_arguments"
            );
            assert_eq!(rejected["result"]["structuredContent"]["executed"], false);
        }
        backend
            .state
            .db
            .with_connection(|c| {
                for table in [
                    "assignments",
                    "plugin_mcp_attempts",
                    "plugin_build_jobs",
                    "codex_conversation",
                    "presence_history",
                ] {
                    let count: i64 =
                        c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))?;
                    assert_eq!(count, 0);
                }
                Ok(())
            })
            .unwrap();
        backend.close().unwrap();
        assert!(!launch.socket.exists());
    }
}
