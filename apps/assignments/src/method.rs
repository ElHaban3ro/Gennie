//! El encargo conserva intención y permisos; el catálogo aporta evidencia técnica vigente.
use crate::{Document, text};
use gennie_core::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Method {
    pub plugin_id: String,
    /// Funciones alternativas para el mismo objetivo, en orden de preferencia.
    pub functions: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Authorization {
    /// Ámbito y efectos expresamente delegados; no sustituye permisos del sistema.
    pub scope: String,
    pub allowed_routes: Vec<Route>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    Http,
    Web,
    ComputerUse,
    Mcp,
}
pub(crate) fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.as_bytes()[0].is_ascii_lowercase()
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}
pub(crate) fn validate(d: &Document) -> Result<()> {
    if let Some(a) = &d.authorization {
        text(&a.scope, 2000, "Autorización")?;
        if a.allowed_routes.is_empty()
            || a.allowed_routes.len() > 4
            || a.allowed_routes
                .iter()
                .enumerate()
                .any(|(i, r)| a.allowed_routes[..i].contains(r))
        {
            return Err("Selecciona vías autorizadas únicas".into());
        }
    }
    if let Some(m) = &d.method {
        if d.authorization.is_none() {
            return Err("El método necesita una autorización explícita".into());
        }
        let mcp = m.plugin_id.strip_prefix("mcp:");
        let valid_plugin = mcp
            .map(identifier)
            .unwrap_or_else(|| identifier(&m.plugin_id));
        if !valid_plugin
            || m.functions.is_empty()
            || m.functions.len() > 20
            || m.functions.iter().any(|f| {
                if mcp.is_some() {
                    f.is_empty() || f.len() > 256 || f.chars().any(char::is_control)
                } else {
                    !identifier(f)
                }
            })
            || m.functions
                .iter()
                .enumerate()
                .any(|(i, f)| m.functions[..i].contains(f))
        {
            return Err("Indica plugin y funciones alternativas únicas válidas".into());
        }
    }
    Ok(())
}
/// Solo evidencia del catálogo, nunca una afirmación de haber ejecutado el encargo.
pub fn resolve(d: &Document, catalogue: &[Value]) -> Option<Value> {
    let method = d.method.as_ref()?;
    let authorization = d.authorization.as_ref()?;
    if method.plugin_id.starts_with("mcp:") {
        let source = catalogue
            .iter()
            .find(|s| s["kind"] == "mcp" && s["id"] == method.plugin_id);
        let Some(source) = source else {
            return Some(
                json!({"status":if catalogue.iter().any(|s|s["kind"]=="mcp_unavailable"){"catalogue_unavailable"}else{"plugin_missing"},"execution_enabled":false}),
            );
        };
        let server = &source["server"];
        let routes=method.functions.iter().map(|name|{
            let present=server["tools"].get(name).is_some();
            let allowed=server["enabled"]==true && authorization.allowed_routes.contains(&Route::Mcp) && server["allowed_tools"].as_array().is_some_and(|tools|tools.iter().any(|t|t==name));
            let schema=server["schema_status"][name]=="ready";
            json!({"function":name,"route":"mcp","allowed":allowed,"verified":false,"status":if !present{"function_missing"}else if !allowed{"not_authorized"}else if !schema{"verification_required"}else{"available"}})
        }).collect::<Vec<_>>();
        let selected = routes.iter().find(|r| r["status"] == "available").cloned();
        return Some(
            json!({"status":if selected.is_some(){"available"}else{"unresolved"},"plugin_revision":server["revision"],"routes":routes,"selected":selected,"execution_enabled":false}),
        );
    }
    let Some(plugin) = catalogue
        .iter()
        .find(|p| p["manifest"]["id"] == method.plugin_id)
    else {
        return Some(
            json!({"status":if catalogue.iter().any(|s|s["kind"]=="web_unavailable"){"catalogue_unavailable"}else{"plugin_missing"},"routes":[],"execution_enabled":false}),
        );
    };
    let revision = &plugin["revision"];
    let functions = plugin["manifest"]["functions"].as_array();
    let routes: Vec<Value> = method.functions.iter().map(|name| {
        let function = functions.and_then(|fs| fs.iter().find(|f| f["name"] == *name));
        let route = match function.and_then(|f| f["transport"].as_str()) {
            Some("http") => Some(Route::Http), Some("page") => Some(Route::Web), _ => None,
        };
        let allowed = route.as_ref().is_some_and(|r| authorization.allowed_routes.contains(r));
        let verified = revision.as_u64().is_some_and(|r| r>0)
            && plugin["verified"][name]["revision"] == *revision
            && plugin["verified"][name]["verified_at"].is_string();
        json!({"function":name,"route":route,"allowed":allowed,"verified":verified,
            "status":if function.is_none(){"function_missing"}else if !allowed{"not_authorized"}else if !verified{"verification_required"}else{"ready"}})
    }).collect();
    let selected = routes.iter().find(|r| r["status"] == "ready").cloned();
    Some(
        json!({"status":if selected.is_some(){"ready"}else{"unresolved"},"plugin_revision":revision,
        "selected":selected,"routes":routes,"execution_enabled":false}),
    )
}
