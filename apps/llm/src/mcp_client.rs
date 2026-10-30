//! Canal de herramientas sin inferencia ni conversación del usuario.
use super::{ProcessHandle, Session, Settings};
use gennie_core::Result;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

pub enum Transport {
    Stdio {
        command: String,
        args: Vec<String>,
        cwd: Option<String>,
        env_vars: Vec<String>,
    },
    Http {
        url: String,
        bearer_token_env_var: Option<String>,
    },
}
fn quoted(value: &str) -> Result<String> {
    Ok(serde_json::to_string(value)?)
}
fn texts(values: &[String]) -> Result<String> {
    Ok(serde_json::to_string(values)?)
}
impl Transport {
    fn config(&self, timeout: u64) -> Result<String> {
        if !(1..=900).contains(&timeout) {
            return Err("Plazo de herramienta MCP inválido".into());
        }
        let specific = match self {
            Self::Stdio {
                command,
                args,
                cwd,
                env_vars,
            } => {
                if command.trim().is_empty()
                    || command.contains('\0')
                    || args.iter().any(|s| s.contains('\0'))
                    || cwd.as_ref().is_some_and(|s| s.contains('\0'))
                {
                    return Err("Configuración de proceso MCP inválida".into());
                }
                format!(
                    "command={},args={},env_vars={}{}",
                    quoted(command)?,
                    texts(args)?,
                    texts(env_vars)?,
                    cwd.as_ref()
                        .map(|s| quoted(s).map(|s| format!(",cwd={s}")))
                        .transpose()?
                        .unwrap_or_default()
                )
            }
            Self::Http {
                url,
                bearer_token_env_var,
            } => format!(
                "url={}{}",
                quoted(url)?,
                bearer_token_env_var
                    .as_ref()
                    .map(|s| quoted(s).map(|s| format!(",bearer_token_env_var={s}")))
                    .transpose()?
                    .unwrap_or_default()
            ),
        };
        Ok(format!(
            "mcp_servers.external={{{specific},startup_timeout_sec=20,tool_timeout_sec={timeout},default_tools_approval_mode=\"approve\"}}"
        ))
    }
}

#[derive(Clone, Default)]
pub struct Control(ProcessHandle);
impl Control {
    pub fn cancel(&self) {
        self.0.cancel();
    }
}
pub struct Client {
    session: Session,
    next_id: i64,
    timeout: Duration,
    usable: bool,
}
impl Client {
    pub fn is_usable(&self) -> bool {
        self.usable
    }
    pub fn connect(
        base: PathBuf,
        transport: &Transport,
        timeout_seconds: u64,
        control: Control,
    ) -> Result<Self> {
        Self::connect_with_context(
            base,
            transport,
            timeout_seconds,
            control,
            gennie_mcp::CallContext::default(),
        )
    }
    pub fn connect_with_context(
        base: PathBuf,
        transport: &Transport,
        timeout_seconds: u64,
        control: Control,
        context: gennie_mcp::CallContext,
    ) -> Result<Self> {
        context.check()?;
        if !base.is_absolute() {
            return Err("La ruta del cliente MCP debe ser absoluta".into());
        }
        let settings = Settings {
            read_files: false,
            write_files: false,
            run_commands: false,
            local_actions: false,
            ..Settings::default()
        };
        let mut session = Session::spawn(
            control.0,
            base,
            &settings,
            None,
            &[transport.config(timeout_seconds)?],
            false,
            Some(context),
        )?;
        session.send(json!({"jsonrpc":"2.0","id":2,"method":"thread/start","params":{"ephemeral":true,"cwd":session.cwd,"permissions":"gennie_voice","approvalPolicy":"never","developerInstructions":"Canal de herramientas de Gennie. No generar turnos de modelo ni iniciar objetivos."}}))?;
        let response = session.response(2, Duration::from_secs(30))?;
        session.thread_id = response
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .ok_or("El proveedor no creó el canal MCP")?
            .into();
        Ok(Self {
            session,
            next_id: 10,
            timeout: Duration::from_secs(timeout_seconds + 15),
            usable: true,
        })
    }
    /// El consumidor posee el mutex del cliente durante toda la operación.
    pub fn bind_context(&mut self, context: Option<gennie_mcp::CallContext>) {
        self.session.cancellation = context;
    }
    fn request(&mut self, method: &str, params: Value, timeout: Duration) -> Result<Value> {
        if !self.usable {
            return Err(
                "La conexión MCP se interrumpió; vuelve a conectar antes de otra operación".into(),
            );
        }
        self.next_id += 1;
        let result = self
            .session
            .send(json!({"jsonrpc":"2.0","id":self.next_id,"method":method,"params":params}))
            .and_then(|_| self.session.response(self.next_id, timeout));
        if result.is_err() {
            self.usable = false;
            self.session.process.kill();
        }
        result
    }
    pub fn inventory(&mut self) -> Result<Value> {
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut pages = 0;
        let mut tools = serde_json::Map::new();
        let mut cursor = None::<String>;
        let mut cursors = std::collections::HashSet::new();
        let mut status = json!({});
        loop {
            pages += 1;
            let remaining = deadline.saturating_duration_since(Instant::now());
            if pages > 20 || remaining.is_zero() {
                return Err(
                    "El inventario MCP excedió su límite de tiempo o páginas; no se ha recortado"
                        .into(),
                );
            }
            let page = self.request("mcpServerStatus/list",json!({"threadId":self.session.thread_id,"serverName":"external","detail":"toolsAndAuthOnly","limit":100,"cursor":cursor}),remaining)?;
            let server = page["data"]
                .as_array()
                .and_then(|items| items.iter().find(|row| row["name"] == "external"))
                .ok_or("El servidor MCP no devolvió su inventario")?;
            if !server["toolsError"].is_null() {
                return Err("No se pudieron descubrir las herramientas del servidor MCP".into());
            }
            let entries = server["tools"]
                .as_object()
                .ok_or("El servidor MCP devolvió un inventario inválido")?;
            for (name, definition) in entries {
                if name.is_empty()
                    || name.chars().count() > 200
                    || name.chars().any(char::is_control)
                    || definition["name"].as_str() != Some(name.as_str())
                    || tools.insert(name.clone(), definition.clone()).is_some()
                {
                    return Err(
                        "El inventario MCP contiene identidades inválidas o duplicadas".into(),
                    );
                }
            }
            if tools.len() > 1000 || serde_json::to_vec(&tools)?.len() > 2 * 1024 * 1024 {
                return Err(
                    "El inventario MCP excede el límite del catálogo; no se ha recortado".into(),
                );
            }
            status["auth_status"] = server["authStatus"].clone();
            cursor = page["nextCursor"].as_str().map(str::to_owned);
            match &cursor {
                None => break,
                Some(value) if !cursors.insert(value.clone()) => {
                    return Err("El servidor MCP repitió un cursor de inventario".into());
                }
                _ => {}
            }
        }
        status["tools"] = Value::Object(tools);
        Ok(status)
    }
    /// No reintenta: un fallo de transporte puede suceder después del efecto externo.
    pub fn call(&mut self, tool: &str, arguments: Value) -> Result<Value> {
        self.request("mcpServer/tool/call",json!({"threadId":self.session.thread_id,"server":"external","tool":tool,"arguments":arguments}),self.timeout)
    }
}

#[cfg(test)]
#[path = "mcp_client_tests.rs"]
mod tests;
