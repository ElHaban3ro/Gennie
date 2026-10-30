//! Configuración propia de Plugins; el cliente de protocolo no posee persistencia.
use super::valid_id;
use gennie_core::Result;
use gennie_llm::mcp_client::Transport;
use serde::{Deserialize, Serialize};
pub const MAX_TOOL_TIMEOUT_SECONDS: u64 = gennie_mcp::MAX_TOOL_EXECUTION_SECS - 180;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Connection {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        cwd: Option<String>,
        #[serde(default)]
        env_vars: Vec<String>,
    },
    Http {
        url: String,
        bearer_token_env_var: Option<String>,
    },
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Specification {
    pub id: String,
    pub name: String,
    pub connection: Connection,
    pub timeout_seconds: u64,
}
fn text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= max && !value.chars().any(char::is_control)
}
fn environment(value: &str) -> bool {
    value.len() <= 128
        && value
            .as_bytes()
            .first()
            .is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_')
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
impl Specification {
    pub fn validate(&self) -> Result<()> {
        if !valid_id(&self.id)
            || !text(&self.name, 200)
            || !(1..=MAX_TOOL_TIMEOUT_SECONDS).contains(&self.timeout_seconds)
        {
            return Err("Identidad, nombre o plazo MCP inválidos".into());
        }
        match &self.connection {
            Connection::Stdio {
                command,
                args,
                cwd,
                env_vars,
            } => {
                if !text(command, 4000)
                    || args.len() > 64
                    || args.iter().any(|a| a.len() > 4000 || a.contains('\0'))
                    || cwd
                        .as_ref()
                        .is_some_and(|p| !text(p, 4000) || !std::path::Path::new(p).is_absolute())
                    || env_vars.len() > 40
                    || env_vars.iter().any(|v| !environment(v))
                {
                    return Err(
                        "Comando, argumentos, carpeta o nombres de variables MCP inválidos".into(),
                    );
                }
            }
            Connection::Http {
                url,
                bearer_token_env_var,
            } => {
                let parsed = reqwest::Url::parse(url).map_err(|_| "Dirección MCP inválida")?;
                let local = parsed.host_str().is_some_and(|host| {
                    host == "localhost"
                        || host
                            .trim_matches(['[', ']'])
                            .parse::<std::net::IpAddr>()
                            .is_ok_and(|ip| ip.is_loopback())
                });
                if !text(url, 4000)
                    || !(parsed.scheme() == "https" || parsed.scheme() == "http" && local)
                    || !parsed.username().is_empty()
                    || parsed.password().is_some()
                    || parsed.query().is_some()
                    || parsed.fragment().is_some()
                    || bearer_token_env_var
                        .as_ref()
                        .is_some_and(|v| !environment(v))
                {
                    return Err("Usa HTTPS (HTTP solo para loopback), sin credenciales/query/fragmento y con nombre de variable de autenticación válido".into());
                }
            }
        }
        if serde_json::to_vec(self)?.len() > 32000 {
            return Err("La configuración MCP excede su límite".into());
        }
        Ok(())
    }
    pub fn transport(&self) -> Transport {
        match &self.connection {
            Connection::Stdio {
                command,
                args,
                cwd,
                env_vars,
            } => Transport::Stdio {
                command: command.clone(),
                args: args.clone(),
                cwd: cwd.clone(),
                env_vars: env_vars.clone(),
            },
            Connection::Http {
                url,
                bearer_token_env_var,
            } => Transport::Http {
                url: url.clone(),
                bearer_token_env_var: bearer_token_env_var.clone(),
            },
        }
    }
}
