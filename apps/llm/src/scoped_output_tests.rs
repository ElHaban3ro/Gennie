//! Proveedor ficticio: respeta el esquema enviado por el transporte real, sin cuenta o red.
use super::*;

pub(crate) fn render(prompt: &str, action: &str, output: ScopedOutput) -> Result<String> {
    let script = r#"
import json, sys
request = json.loads(sys.stdin.readline())
assert request['method'] == 'turn/start'
params = request['params']
assert params['input'][0]['text']
schema = params['outputSchema']
assert schema['additionalProperties'] is False
assert sorted(schema['required']) == sorted(schema['properties'])
output = {'message': 'Mensaje exclusivo de Gennie'}
if 'action' in schema['properties']:
    action = sys.argv[1]
    assert action in schema['properties']['action']['enum']
    assert schema['properties']['message']['type'] == ['string', 'null']
    output.update(action=action, reason='Necesidad concreta según el acuerdo')
    if action == 'record': output['message'] = None
assert set(output) == set(schema['properties'])
def emit(value): print(json.dumps(value), flush=True)
emit({'id': request['id'], 'result': {'turn': {'id': 'fixture-turn'}}})
emit({'method': 'item/completed', 'params': {'threadId': params['threadId'], 'turnId': 'fixture-turn', 'item': {'type': 'agentMessage', 'id': 'reply', 'text': json.dumps(output)}}})
emit({'method': 'turn/completed', 'params': {'turn': {'id': 'fixture-turn', 'status': 'completed'}}})
"#;
    let mut child = Command::new("python3")
        .args(["-c", script, action])
        .process_group(0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let stdin = child.stdin.take().ok_or("Falta stdin del fixture")?;
    let stdout = child.stdout.take().ok_or("Falta stdout del fixture")?;
    let process = ProcessHandle::default();
    *process.0.lock().unwrap() = Some(child);
    let cleanup = process.clone();
    let (sender, lines) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if sender.send(line.unwrap()).is_err() {
                break;
            }
        }
    });
    let mut session = Session {
        stdin,
        lines,
        reader: Some(reader),
        process,
        thread_id: "unused-conversation".into(),
        journal: None,
        cwd: std::env::temp_dir(),
        model: "fixture".into(),
        effort: "low".into(),
        title_effort: None,
        settings: Settings::default(),
        tool_names: Vec::new(),
        has_mcp: false,
        cancellation: None,
        ticket: None,
    };
    let result = session.render_scoped_turn("isolated-bot", prompt, output);
    drop(session);
    assert!(cleanup.0.lock().unwrap().is_none());
    result
}

#[test]
fn ordinary_scoped_output_remains_message_only() {
    let text = render("Observación ficticia", "unused", ScopedOutput::Message).unwrap();
    assert_eq!(
        crate::delegation::validate_rendered(&text).unwrap(),
        "Mensaje exclusivo de Gennie"
    );
}
