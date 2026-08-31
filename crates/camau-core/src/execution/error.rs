use crate::json_value::JsonValue;

#[derive(Debug, thiserror::Error)]
#[error("mapping at node {node_id:?} could not produce target {target:?} ({reason})")]
pub struct MappingFailure {
    pub node_id: String,
    pub target: String,
    pub source_path: Option<String>,
    pub expected_type: &'static str,
    pub reason: &'static str,
}

#[derive(Clone, Debug)]
pub struct GateSelectionContext {
    pub gate_node_id: String,
    pub selected_path: Option<String>,
    pub value_found: Option<bool>,
    pub selected_value: Option<JsonValue>,
    pub matched_operator: Option<String>,
}

#[derive(Debug, thiserror::Error)]
#[error("routing stopped at node {node_id:?}: {message}")]
pub struct RoutingFailure {
    pub node_id: String,
    pub message: String,
    pub gate: Option<GateSelectionContext>,
}

#[derive(Debug, thiserror::Error)]
pub enum ExecutionFailure {
    #[error(transparent)]
    Mapping(#[from] MappingFailure),
    #[error(transparent)]
    Routing(Box<RoutingFailure>),
    #[error("execution has already started")]
    AlreadyStarted,
    #[error("unknown or completed task ticket")]
    UnknownTicket,
}

impl From<RoutingFailure> for ExecutionFailure {
    fn from(failure: RoutingFailure) -> Self {
        Self::Routing(Box::new(failure))
    }
}
