//! Distingue trabajo pendiente de una propuesta de detención, sin plazos de construcción.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::hash::{Hash, Hasher};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum BlockKind {
    WorkRemaining,
    RecoverableFailure,
    UserActionRequired,
    PermissionRequired,
    ExhaustedAlternatives,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum OutcomeStatus {
    Unmet,
    AlreadySatisfied,
    NotRequired,
    Unclear,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Research {
    sources: Vec<String>,
    findings: String,
    #[serde(default)]
    unavailable_reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Blocker {
    kind: BlockKind,
    capability: Option<String>,
    required_outcome: Option<String>,
    outcome_status: Option<OutcomeStatus>,
    evidence: String,
    next_step: String,
    #[serde(default)]
    attempted_approaches: Vec<String>,
    research: Option<Research>,
    #[serde(default)]
    observation_ids: Vec<String>,
}

impl Blocker {
    fn same_obstacle(&self, other: &Self) -> bool {
        self.kind == other.kind && self.capability == other.capability
    }
}

struct Proposal {
    blocker: Blocker,
    turn: u64,
    sequence: u64,
}
struct Focus {
    capability: String,
    hypothesis: String,
}
struct Observation {
    id: String,
    sequence: u64,
    capability: String,
    hypothesis: String,
    tool: String,
    turn: u64,
    delivered: bool,
    novel: bool,
    fingerprint: u64,
}
#[derive(Default)]
pub(crate) struct BuildStrategy {
    turn: u64,
    sequence: u64,
    focus: Option<Focus>,
    observations: Vec<Observation>,
    proposal: Option<Proposal>,
}

fn technical_key(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}
fn normalized(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .filter(|(key, _)| {
                    !matches!(
                        key.as_str(),
                        "ref"
                            | "target"
                            | "snapshot_id"
                            | "page_id"
                            | "artifact"
                            | "tabs"
                            | "build_observation"
                    )
                })
                .map(|(key, value)| (key.clone(), normalized(value)))
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(normalized).collect()),
        _ => value.clone(),
    }
}
fn ui_tool(tool: &str) -> bool {
    matches!(
        tool,
        "browser_snapshot"
            | "browser_screenshot"
            | "browser_read"
            | "browser_find"
            | "browser_click"
            | "browser_fill"
            | "browser_press"
            | "browser_hover"
            | "browser_scroll"
            | "browser_select_option"
            | "browser_check"
    )
}
fn inspection_tool(tool: &str) -> bool {
    matches!(
        tool,
        "browser_snapshot" | "browser_screenshot" | "browser_read" | "browser_find"
    )
}
pub(crate) enum BlockDecision {
    Continue(&'static str),
    Review,
    Stop,
}

impl BuildStrategy {
    pub fn begin_turn(&mut self) {
        self.turn += 1;
    }

    pub fn investigate(&mut self, capability: &str, hypothesis: &str) -> Result<(), &'static str> {
        if !technical_key(capability) || !technical_key(hypothesis) {
            return Err(
                "Usa identificadores técnicos estables para capacidad e hipótesis (ASCII minúsculas y guion bajo), sin contenido personal",
            );
        }
        self.focus = Some(Focus {
            capability: capability.into(),
            hypothesis: hypothesis.into(),
        });
        Ok(())
    }
    /// Registra solo procedencia y huellas en memoria, no contenido privado de la pantalla.
    pub fn observe(&mut self, tool: &str, args: &Value, result: Option<&Value>) -> Option<Value> {
        if !ui_tool(tool) {
            return None;
        }
        let focus = self.focus.as_ref()?;
        let delivered = result.is_some_and(|r| match tool {
            "browser_snapshot" => r["open"] != false && r["snapshot"].is_string(),
            "browser_screenshot" => r
                .pointer("/__gennie_mcp_output_v1/image/data")
                .is_some_and(Value::is_string),
            _ => true,
        });
        let pixels = result.and_then(|r| r.pointer("/__gennie_mcp_output_v1/image/data"));
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        normalized(args).to_string().hash(&mut hasher);
        if let Some(pixels) = pixels {
            pixels.as_str().unwrap_or_default().hash(&mut hasher);
        } else {
            normalized(result.unwrap_or(&Value::Null))
                .to_string()
                .hash(&mut hasher);
        }
        let fingerprint = hasher.finish();
        let novel = delivered
            && !self.observations.iter().any(|o| {
                o.capability == focus.capability
                    && o.tool == tool
                    && o.delivered
                    && o.fingerprint == fingerprint
            });
        self.sequence += 1;
        let id = format!("obs_{}", self.sequence);
        self.observations.push(Observation {
            id: id.clone(),
            sequence: self.sequence,
            capability: focus.capability.clone(),
            hypothesis: focus.hypothesis.clone(),
            tool: tool.into(),
            turn: self.turn,
            delivered,
            novel,
            fingerprint,
        });
        Some(
            json!({"id":id,"capability":focus.capability,"hypothesis":focus.hypothesis,"tool":tool,"delivered":delivered,"novel":novel}),
        )
    }
    pub fn candidate_changed(&mut self) {
        self.proposal = None;
        self.observations.clear();
    }

    pub fn review_context(&self) -> Value {
        self.proposal.as_ref().map_or(Value::Null, |p| {
            serde_json::json!({
                "proposed_blocker":p.blocker,
                "observations":self.observations.iter().filter(|o| Some(&o.capability) == p.blocker.capability.as_ref()).map(|o|json!({"id":o.id,"hypothesis":o.hypothesis,"tool":o.tool,"delivered":o.delivered,"novel":o.novel,"turn":o.turn})).collect::<Vec<_>>(),
                "instruction":"Revisa críticamente esta capacidad y este resultado solicitado. Comprueba que el requisito procede del encargo y no de una interpretación añadida. La ausencia de un botón no prueba que falte el efecto: observa si ya se cumple. Si el alcance es ambiguo y cambia los efectos, pregunta al usuario. Que falte desarrollar o verificar es trabajo pendiente. Haz una comprobación nueva de lectura e investiga una vía distinta si procede. Si corriges el candidato o cambia el obstáculo, esta revisión deja de ser válida. Continúa mientras haya un paso viable; no repitas escrituras inciertas."
            })
        })
    }

    pub fn assess(&mut self, value: &Value, turn_budget_exhausted: bool) -> BlockDecision {
        if turn_budget_exhausted {
            return BlockDecision::Continue(
                "El presupuesto es solo de este turno, no del trabajo. Termina el turno; el host continuará con un presupuesto nuevo. No lo declares bloqueo.",
            );
        }
        let Ok(blocker) = serde_json::from_value::<Blocker>(value.clone()) else {
            return BlockDecision::Continue(
                "Falta un impedimento concreto y estructurado. Si queda implementar o comprobar una capacidad, continúa. Usa blocker con kind, evidence y next_step solo para proponer una detención real.",
            );
        };
        if matches!(
            blocker.kind,
            BlockKind::WorkRemaining | BlockKind::RecoverableFailure
        ) {
            self.proposal = None;
            return BlockDecision::Continue(
                "Esto es trabajo pendiente o un fallo recuperable. Conserva los hallazgos, descompón lo que falta, corrige la función, busca documentación pública y prueba una alternativa pertinente. Continúa sin repetir efectos inciertos.",
            );
        }
        if blocker.evidence.trim().is_empty() || blocker.next_step.trim().is_empty() {
            return BlockDecision::Continue(
                "Explica el obstáculo observado y la intervención o condición concreta necesaria. Una función incompleta no justifica detener el trabajo.",
            );
        }
        if !blocker.capability.as_ref().is_some_and(|s| {
            !s.is_empty()
                && s.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        }) {
            return BlockDecision::Continue(
                "Identifica en blocker.capability la capacidad concreta con un identificador técnico estable (ASCII minúsculas y guion bajo), no el objetivo completo. Cada impedimento necesita su propia revisión.",
            );
        }
        if blocker.kind == BlockKind::ExhaustedAlternatives {
            if blocker
                .required_outcome
                .as_ref()
                .is_none_or(|s| s.trim().is_empty())
                || blocker.outcome_status.is_none()
            {
                return BlockDecision::Continue(
                    "Antes de detenerte, concreta required_outcome: el efecto pedido por el usuario, no un botón ni una interpretación adicional. Observa y declara outcome_status: unmet, already_satisfied, not_required o unclear.",
                );
            }
            if blocker.outcome_status != Some(OutcomeStatus::Unmet) {
                self.proposal = None;
                return BlockDecision::Continue(
                    "El efecto ya se cumple, no forma parte del encargo o falta aclarar su significado. No declares agotadas las alternativas: verifica la poscondición sin modificar de nuevo, continúa el alcance pedido o formula una pregunta breve mediante waiting_user con la interpretación por aclarar. No amplíes el objetivo.",
                );
            }
            let distinct: std::collections::HashSet<_> = blocker
                .attempted_approaches
                .iter()
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            let researched = blocker.research.as_ref().is_some_and(|r| {
                !r.findings.trim().is_empty()
                    && ((!r.sources.is_empty()
                        && r.sources.iter().all(|s| {
                            reqwest::Url::parse(s).is_ok_and(|u| {
                                matches!(u.scheme(), "http" | "https")
                                    && u.host_str().is_some()
                                    && u.username().is_empty()
                                    && u.password().is_none()
                            })
                        }))
                        || r.unavailable_reason
                            .as_ref()
                            .is_some_and(|s| !s.trim().is_empty()))
            });
            if distinct.len() < 2 || !researched {
                return BlockDecision::Continue(
                    "Antes de agotar alternativas, comprueba vías técnicas distintas e investiga en internet documentación y soluciones. Informa attempted_approaches y research con fuentes consultadas y hallazgos, o el impedimento observado para investigar. No inventes consultas ni fuentes.",
                );
            }
        }
        let needs_ui = matches!(
            blocker.kind,
            BlockKind::ExhaustedAlternatives | BlockKind::PermissionRequired
        );
        let relevant: Vec<_> = self
            .observations
            .iter()
            .filter(|o| {
                blocker.observation_ids.contains(&o.id)
                    && Some(&o.capability) == blocker.capability.as_ref()
            })
            .collect();
        if needs_ui {
            let ids: std::collections::HashSet<_> = blocker.observation_ids.iter().collect();
            if ids.len() != relevant.len() || ids.is_empty() {
                return BlockDecision::Continue(
                    "Vincula el bloqueo a observation_ids reales emitidos por el host para esta capacidad y candidato. Usa plugin_build_investigate antes de explorar; no sirven IDs inventados, de otra capacidad o de una revisión anterior.",
                );
            }
            let dom = relevant
                .iter()
                .any(|o| o.tool == "browser_snapshot" && o.delivered);
            let visual_attempt = relevant.iter().any(|o| o.tool == "browser_screenshot");
            let explored = relevant.iter().any(|o| {
                o.delivered
                    && !matches!(
                        o.tool.as_str(),
                        "browser_snapshot" | "browser_screenshot" | "browser_read"
                    )
            });
            if !(dom && visual_attempt && explored) {
                return BlockDecision::Continue(
                    "Falta investigar la interfaz de esta capacidad con DOM entregado, captura contextual y exploración de controles. API/request/network no sustituyen esa evidencia. Una captura fallida cuenta solo como fallo visual: continúa por DOM y describe el fallo, sin afirmar que viste la imagen.",
                );
            }
        }
        if let Some(p) = self
            .proposal
            .as_ref()
            .filter(|p| p.blocker.same_obstacle(&blocker))
        {
            let fresh = relevant.iter().any(|o| {
                o.turn == self.turn
                    && o.sequence > p.sequence
                    && o.delivered
                    && o.novel
                    && inspection_tool(&o.tool)
            });
            if self.turn > p.turn && (!needs_ui || fresh) {
                return BlockDecision::Stop;
            }
            if self.turn > p.turn && !fresh {
                return BlockDecision::Continue(
                    "La revisión necesita una observación UI nueva y pertinente de esta capacidad. Repetir la misma lectura, un error, otra API o una prueba ajena no confirma el bloqueo. Investiga una hipótesis distinta, observa el resultado y cita su observation_id.",
                );
            }
        } else {
            self.proposal = Some(Proposal {
                blocker,
                turn: self.turn,
                sequence: self.sequence,
            });
        }
        BlockDecision::Review
    }
}

/// Contrato único del reporte que consume el constructor.
pub(crate) fn report_definition() -> Value {
    serde_json::json!({
        "name": "plugin_build_report",
        "description": "Informa avances o un candidato listo. blocked/waiting_user son propuestas revisadas por el host: requieren blocker y no detienen por sí solas el trabajo. Trabajo pendiente o fallos recuperables deben continuar. Una propuesta real necesita otra revisión y evidencia nueva antes de aceptarse. recommended_route no concede permisos.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "state": {
                    "type": "string",
                    "enum": [
                        "progress",
                        "ready",
                        "waiting_user",
                        "blocked"
                    ]
                },
                "message": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 600
                },
                "recommended_route": {
                    "type": "string",
                    "enum": [
                        "http",
                        "web",
                        "computer_use"
                    ]
                },
                "blocker": {
                    "type": "object",
                    "properties": {
                        "kind": {
                            "type": "string",
                            "enum": [
                                "work_remaining",
                                "recoverable_failure",
                                "user_action_required",
                                "permission_required",
                                "exhausted_alternatives"
                            ]
                        },
                        "capability": {
                            "type":"string",
                            "description":"Identificador estable de la capacidad concreta, como paginate_results; no el objetivo completo.",
                            "minLength":1
                        },
                        "required_outcome": {
                            "type":"string",
                            "description":"Efecto observable pedido por el usuario; no añadir requisitos ni exigir un botón concreto.",
                            "minLength":1
                        },
                        "outcome_status": {
                            "type":"string",
                            "enum":["unmet","already_satisfied","not_required","unclear"]
                        },
                        "evidence": {
                            "type": "string",
                            "minLength": 1
                        },
                        "next_step": {
                            "type": "string",
                            "minLength": 1
                        },
                        "observation_ids": {"type":"array","items":{"type":"string"},"description":"IDs reales del host para la capacidad y candidato actuales; incluye los de la revisión nueva."},
                        "attempted_approaches": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            }
                        },
                        "research": {
                            "type": "object",
                            "properties": {
                                "sources": {
                                    "type": "array",
                                    "items": {
                                        "type": "string"
                                    }
                                },
                                "findings": {
                                    "type": "string"
                                },
                                "unavailable_reason": {
                                    "type": "string"
                                }
                            },
                            "required": [
                                "sources",
                                "findings"
                            ],
                            "additionalProperties": false
                        }
                    },
                    "required": [
                        "kind",
                        "evidence",
                        "next_step"
                    ],
                    "additionalProperties": false
                }
            },
            "required": [
                "state",
                "message"
            ],
            "additionalProperties": false
        }
    })
}
