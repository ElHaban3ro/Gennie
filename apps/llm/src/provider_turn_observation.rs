//! Observación optativa y acotada del protocolo en sondas cfg(test).
use gennie_core::Result;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{HashSet, VecDeque},
};

const MAX_EVENTS: usize = 96;

#[derive(Default)]
struct Observation {
    events_seen: u64,
    events: VecDeque<Value>,
    collaboration: Vec<Value>,
    root_thread: Option<String>,
    root_children: HashSet<String>,
}
thread_local! {
    static ACTIVE: RefCell<Option<Observation>> = const { RefCell::new(None) };
}

struct Scope {
    previous: Option<Observation>,
    finished: bool,
}
impl Scope {
    fn begin() -> Self {
        Self {
            previous: ACTIVE.with(|slot| slot.replace(Some(Observation::default()))),
            finished: false,
        }
    }
    fn finish(mut self) -> Value {
        let trace = ACTIVE
            .with(|slot| slot.replace(self.previous.take()))
            .unwrap_or_default();
        self.finished = true;
        json!({"events_seen":trace.events_seen,
            "events_dropped":trace.events_seen.saturating_sub(trace.events.len() as u64),
            "events":trace.events,"collaboration":trace.collaboration,
            "root_children_observed":trace.root_children.len()})
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        if !self.finished {
            ACTIVE.with(|slot| slot.replace(self.previous.take()));
        }
    }
}

pub(super) fn capture<T>(run: impl FnOnce() -> Result<T>) -> (Result<T>, Value) {
    let scope = Scope::begin();
    let result = run();
    (result, scope.finish())
}

pub(super) fn capture_with_root<T>(
    root: &str,
    run: impl FnOnce() -> Result<T>,
) -> (Result<T>, Value) {
    let scope = Scope::begin();
    ACTIVE.with(|slot| {
        if let Some(trace) = slot.borrow_mut().as_mut() {
            trace.root_thread = Some(root.to_owned());
        }
    });
    let result = run();
    (result, scope.finish())
}

pub(super) fn record(parsed: &Result<Value>) {
    ACTIVE.with(|slot| {
        let mut active = slot.borrow_mut();
        let Some(trace) = active.as_mut() else { return };
        trace.events_seen = trace.events_seen.saturating_add(1);
        if let Ok(event) = parsed {
            let kind = event.pointer("/params/item/type").and_then(Value::as_str);
            let method = event.get("method").and_then(Value::as_str);
            if matches!(kind, Some("collabAgentToolCall" | "subAgentActivity"))
                && matches!(method, Some("item/started" | "item/completed"))
                && trace.collaboration.len() < 96
            {
                if event.pointer("/params/threadId").and_then(Value::as_str)
                    == trace.root_thread.as_deref()
                    && kind == Some("subAgentActivity")
                    && let Some(child) = event
                        .pointer("/params/item/agentThreadId")
                        .and_then(Value::as_str)
                    && Some(child) != trace.root_thread.as_deref()
                    && trace.root_children.len() < 32
                {
                    trace.root_children.insert(child.to_owned());
                }
                let tool = event
                    .pointer("/params/item/tool")
                    .and_then(Value::as_str)
                    .filter(|tool| {
                        matches!(*tool, "spawnAgent" | "wait" | "sendInput" | "closeAgent")
                    });
                trace
                    .collaboration
                    .push(json!({"item_type":kind,"method":method,"tool":tool}));
            }
        }
        if trace.events.len() == MAX_EVENTS {
            trace.events.pop_front();
        }
        trace.events.push_back(project(parsed));
    });
}

fn project(parsed: &Result<Value>) -> Value {
    let Ok(event) = parsed else {
        return json!({"kind":"parse_failure"});
    };
    let kind = match (event.get("id").is_some(), event.get("method").is_some()) {
        (true, true) => "server_request",
        (true, false) => "rpc_response",
        _ => "notification",
    };
    // Lista cerrada: no se copian nombres desconocidos ni valores libres de params.
    let method = match event["method"].as_str() {
        Some(
            value @ ("item/tool/requestUserInput"
            | "item/commandExecution/requestApproval"
            | "item/fileChange/requestApproval"
            | "item/tool/call"
            | "account/chatgptAuthTokens/refresh"
            | "item/started"
            | "item/completed"
            | "item/agentMessage/delta"
            | "turn/started"
            | "turn/completed"
            | "thread/name/updated"
            | "thread/tokenUsage/updated"
            | "error"),
        ) => value,
        Some(_) => "other",
        None => "absent_or_invalid",
    };
    let mut result = json!({"kind":kind,"method":method});
    if let Some(status) = event.pointer("/params/turn/status") {
        result["turn_status"] = json!(
            status
                .as_str()
                .filter(|value| matches!(
                    *value,
                    "completed" | "failed" | "interrupted" | "inProgress"
                ))
                .unwrap_or("other")
        );
    }
    if let Some(item_type) = event.pointer("/params/item/type") {
        result["item_type"] = json!(
            item_type
                .as_str()
                .filter(|value| matches!(
                    *value,
                    "agentMessage"
                        | "collabAgentToolCall"
                        | "subAgentActivity"
                        | "mcpToolCall"
                        | "reasoning"
                        | "commandExecution"
                        | "fileChange"
                ))
                .unwrap_or("other")
        );
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turn_observation_bounds_records_and_discards_unknown_names_and_values() {
        let event = Ok(json!({"id":"PRIVATE_ID","method":"PRIVATE_METHOD",
            "params":{"item":{"type":"PRIVATE_TYPE"},"turn":{"status":"PRIVATE_STATUS"},
                "token":"PRIVATE_TOKEN","text":"PRIVATE_TEXT"}}));
        let (_, trace) = capture(|| {
            for _ in 0..200 {
                record(&event);
            }
            Ok(())
        });
        assert_eq!(trace["events_seen"], 200);
        assert_eq!(trace["events_dropped"], 104);
        assert_eq!(trace["events"].as_array().unwrap().len(), MAX_EVENTS);
        assert_eq!(trace["events"][0]["method"], "other");
        assert!(!trace.to_string().contains("PRIVATE"));
        let (_, trace) = capture(|| {
            for _ in 0..200 {
                record(&event);
            }
            record(&Ok(json!({"id":1,"method":"item/tool/requestUserInput"})));
            Ok(())
        });
        assert_eq!(
            trace["events"].as_array().unwrap().last().unwrap()["method"],
            "item/tool/requestUserInput"
        );
    }

    #[test]
    fn collaboration_survives_delta_saturation_without_private_fields() {
        let (_, trace) = capture(|| {
            record(&Ok(json!({"method":"item/completed","params":{"item":{
                "type":"subAgentActivity","agentThreadId":"PRIVATE_ID","message":"PRIVATE_TEXT"}}})));
            for _ in 0..200 {
                record(&Ok(json!({"method":"item/agentMessage/delta"})));
            }
            Ok(())
        });
        assert_eq!(trace["collaboration"].as_array().unwrap().len(), 1);
        assert!(trace["events_dropped"].as_u64().unwrap() > 0);
        assert!(!trace.to_string().contains("PRIVATE"));
    }

    #[test]
    fn project_observation_counts_distinct_children_without_serializing_ids() {
        let (_, trace) = capture_with_root("ROOT_PRIVATE", || {
            for child in [
                "CHILD_ONE_PRIVATE",
                "CHILD_TWO_PRIVATE",
                "CHILD_ONE_PRIVATE",
            ] {
                record(&Ok(
                    json!({"method":"item/started","params":{"threadId":"ROOT_PRIVATE",
                    "item":{"type":"subAgentActivity","agentThreadId":child}}}),
                ));
            }
            record(&Ok(
                json!({"method":"item/started","params":{"threadId":"OTHER_PRIVATE",
                "item":{"type":"subAgentActivity","agentThreadId":"THIRD_PRIVATE"}}}),
            ));
            Ok(())
        });
        assert_eq!(trace["root_children_observed"], 2);
        assert!(!trace.to_string().contains("PRIVATE"));
    }

    #[test]
    fn turn_observation_is_opt_in_thread_local_and_restores_scope_after_panic() {
        let event = Ok(json!({"method":"item/completed"}));
        record(&event);
        let (_, empty) = capture(|| Ok(()));
        assert_eq!(empty["events_seen"], 0);
        let (_, outer) = capture(|| {
            record(&event);
            let (_, nested) = capture(|| {
                record(&event);
                Ok(())
            });
            assert_eq!(nested["events_seen"], 1);
            let child = std::thread::spawn(|| capture(|| Ok(())).1).join().unwrap();
            assert_eq!(child["events_seen"], 0);
            let _ = std::panic::catch_unwind(|| {
                let _ = capture::<()>(|| {
                    record(&Ok(json!({"method":"item/completed"})));
                    panic!("Fixture");
                });
            });
            record(&event);
            Ok(())
        });
        assert_eq!(outer["events_seen"], 2);
        let (_, empty) = capture(|| Ok(()));
        assert_eq!(empty["events_seen"], 0);
    }
}
