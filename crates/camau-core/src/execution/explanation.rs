use indexmap::IndexMap;

use crate::graph::{GateCase, JsonNumber};
use crate::json_parsing::JsonValue;

pub(super) struct Explanation {
    objects: Vec<JsonValue>,
    visits: Vec<NodeVisit>,
    outcome: Option<Outcome>,
}

pub(super) struct ConvergenceReasonInput {
    pub flow_id: String,
    pub from_node: String,
}

pub(super) enum VisitReason {
    Entry,
    Next {
        from_node: String,
    },
    DeterministicGate {
        from_node: String,
        selected_path: String,
        value_found: bool,
        selected_value: Option<JsonValue>,
        matched: GateMatch,
    },
    RandomisedGate {
        from_node: String,
        probability: f64,
    },
    FanOut {
        from_node: String,
        flow_id: String,
    },
    Convergence {
        inputs: Vec<ConvergenceReasonInput>,
    },
}

pub(super) enum GateMatch {
    Eq(JsonValue),
    Lt(JsonNumber),
    Le(JsonNumber),
    Ge(JsonNumber),
    Gt(JsonNumber),
    Range(JsonNumber, JsonNumber),
    Set(Vec<String>),
    Otherwise,
}

struct NodeVisit {
    node_id: String,
    node_type: &'static str,
    reason: VisitReason,
    input_object_ids: Vec<usize>,
    output_object_id: Option<usize>,
    status: VisitStatus,
}

enum VisitStatus {
    Active,
    Completed,
    Terminal,
    Abandoned,
}

enum Outcome {
    Success {
        output_object_id: usize,
    },
    RaiseError {
        node_id: String,
        message: String,
        current_object_id: usize,
    },
}

impl Explanation {
    pub(super) fn new(payload: &JsonValue) -> Self {
        Self {
            objects: vec![payload.clone()],
            visits: Vec::new(),
            outcome: None,
        }
    }

    pub(super) fn add_object(&mut self, value: &JsonValue) -> usize {
        let id = self.objects.len();
        self.objects.push(value.clone());
        id
    }

    pub(super) fn begin_visit(
        &mut self,
        node_id: String,
        node_type: &'static str,
        reason: VisitReason,
        input_object_ids: Vec<usize>,
    ) -> usize {
        let index = self.visits.len();
        self.visits.push(NodeVisit {
            node_id,
            node_type,
            reason,
            input_object_ids,
            output_object_id: None,
            status: VisitStatus::Active,
        });
        index
    }

    pub(super) fn complete_visit(&mut self, index: usize, output_object_id: usize) {
        let visit = &mut self.visits[index];
        visit.output_object_id = Some(output_object_id);
        visit.status = VisitStatus::Completed;
    }

    pub(super) fn record_completed_visit(
        &mut self,
        node_id: String,
        node_type: &'static str,
        reason: VisitReason,
        input_object_ids: Vec<usize>,
        output_object_id: usize,
    ) {
        let index = self.begin_visit(node_id, node_type, reason, input_object_ids);
        self.complete_visit(index, output_object_id);
    }

    pub(super) fn record_terminal_visit(
        &mut self,
        node_id: String,
        node_type: &'static str,
        reason: VisitReason,
        input_object_ids: Vec<usize>,
    ) {
        let index = self.begin_visit(node_id, node_type, reason, input_object_ids);
        self.visits[index].status = VisitStatus::Terminal;
    }

    pub(super) fn abandon_visit(&mut self, index: usize) {
        self.visits[index].status = VisitStatus::Abandoned;
    }

    pub(super) fn finish_success(&mut self, output_object_id: usize) {
        self.outcome = Some(Outcome::Success { output_object_id });
    }

    pub(super) fn finish_raise_error(
        &mut self,
        node_id: String,
        message: String,
        current_object_id: usize,
    ) {
        self.outcome = Some(Outcome::RaiseError {
            node_id,
            message,
            current_object_id,
        });
    }

    pub(super) fn finished(&self) -> bool {
        self.outcome.is_some()
    }

    pub(super) fn into_json(self) -> Option<JsonValue> {
        let outcome = self.outcome?;
        Some(object([
            (
                "objects",
                JsonValue::Array(
                    self.objects
                        .into_iter()
                        .enumerate()
                        .map(|(id, value)| object([("id", integer(id)), ("value", value)]))
                        .collect(),
                ),
            ),
            (
                "visits",
                JsonValue::Array(self.visits.into_iter().map(NodeVisit::into_json).collect()),
            ),
            ("outcome", outcome.into_json()),
        ]))
    }
}

impl VisitReason {
    pub(super) fn deterministic_gate(
        from_node: String,
        selected_path: String,
        selected_value: Option<JsonValue>,
        case: &GateCase,
    ) -> Self {
        Self::DeterministicGate {
            from_node,
            selected_path,
            value_found: selected_value.is_some(),
            selected_value,
            matched: GateMatch::from(case),
        }
    }

    fn into_json(self) -> JsonValue {
        match self {
            Self::Entry => object([("kind", string("entry"))]),
            Self::Next { from_node } => object([
                ("kind", string("next")),
                ("from_node", JsonValue::String(from_node)),
            ]),
            Self::DeterministicGate {
                from_node,
                selected_path,
                value_found,
                selected_value,
                matched,
            } => object([
                ("kind", string("deterministic-gate")),
                ("from_node", JsonValue::String(from_node)),
                ("selected_path", JsonValue::String(selected_path)),
                ("value_found", JsonValue::Bool(value_found)),
                ("selected_value", selected_value.unwrap_or(JsonValue::Null)),
                ("matched", matched.into_json()),
            ]),
            Self::RandomisedGate {
                from_node,
                probability,
            } => object([
                ("kind", string("randomised-gate")),
                ("from_node", JsonValue::String(from_node)),
                ("probability", JsonValue::Number(probability)),
            ]),
            Self::FanOut { from_node, flow_id } => object([
                ("kind", string("fan-out")),
                ("from_node", JsonValue::String(from_node)),
                ("flow_id", JsonValue::String(flow_id)),
            ]),
            Self::Convergence { inputs } => object([
                ("kind", string("convergence")),
                (
                    "inputs",
                    JsonValue::Array(
                        inputs
                            .into_iter()
                            .map(|input| {
                                object([
                                    ("flow_id", JsonValue::String(input.flow_id)),
                                    ("from_node", JsonValue::String(input.from_node)),
                                ])
                            })
                            .collect(),
                    ),
                ),
            ]),
        }
    }
}

impl From<&GateCase> for GateMatch {
    fn from(case: &GateCase) -> Self {
        match case {
            GateCase::Eq { value, .. } => Self::Eq(value.clone()),
            GateCase::Lt { value, .. } => Self::Lt(*value),
            GateCase::Le { value, .. } => Self::Le(*value),
            GateCase::Ge { value, .. } => Self::Ge(*value),
            GateCase::Gt { value, .. } => Self::Gt(*value),
            GateCase::Range { lower, upper, .. } => Self::Range(*lower, *upper),
            GateCase::Set { values, .. } => Self::Set(values.clone()),
            GateCase::Otherwise { .. } => Self::Otherwise,
        }
    }
}

impl GateMatch {
    fn into_json(self) -> JsonValue {
        match self {
            Self::Eq(value) => object([("operator", string("eq")), ("value", value)]),
            Self::Lt(value) => number_match("lt", value),
            Self::Le(value) => number_match("le", value),
            Self::Ge(value) => number_match("ge", value),
            Self::Gt(value) => number_match("gt", value),
            Self::Range(lower, upper) => object([
                ("operator", string("in_range")),
                ("lower", json_number(lower)),
                ("upper", json_number(upper)),
            ]),
            Self::Set(values) => object([
                ("operator", string("in_set")),
                (
                    "values",
                    JsonValue::Array(values.into_iter().map(JsonValue::String).collect()),
                ),
            ]),
            Self::Otherwise => object([("operator", string("otherwise"))]),
        }
    }
}

impl NodeVisit {
    fn into_json(self) -> JsonValue {
        let mut value = IndexMap::from([
            ("node_id".to_owned(), JsonValue::String(self.node_id)),
            ("node_type".to_owned(), string(self.node_type)),
            ("reason".to_owned(), self.reason.into_json()),
            (
                "input_object_ids".to_owned(),
                JsonValue::Array(self.input_object_ids.into_iter().map(integer).collect()),
            ),
        ]);
        if let Some(output_object_id) = self.output_object_id {
            value.insert("output_object_id".to_owned(), integer(output_object_id));
        }
        value.insert("status".to_owned(), string(self.status.name()));
        JsonValue::Object(value)
    }
}

impl VisitStatus {
    fn name(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Terminal => "terminal",
            Self::Abandoned => "abandoned",
        }
    }
}

impl Outcome {
    fn into_json(self) -> JsonValue {
        match self {
            Self::Success { output_object_id } => object([
                ("kind", string("success")),
                ("output_object_id", integer(output_object_id)),
            ]),
            Self::RaiseError {
                node_id,
                message,
                current_object_id,
            } => object([
                ("kind", string("raise-error")),
                ("code", string("GWALL")),
                ("node_id", JsonValue::String(node_id)),
                ("message", JsonValue::String(message)),
                ("current_object_id", integer(current_object_id)),
            ]),
        }
    }
}

fn number_match(operator: &'static str, value: JsonNumber) -> JsonValue {
    object([
        ("operator", string(operator)),
        ("value", json_number(value)),
    ])
}

fn json_number(value: JsonNumber) -> JsonValue {
    match value {
        JsonNumber::Integer(value) => JsonValue::Integer(value),
        JsonNumber::Float(value) => JsonValue::Number(value),
    }
}

fn integer(value: usize) -> JsonValue {
    JsonValue::Integer(i64::try_from(value).expect("explanation index exceeds signed 64-bit range"))
}

fn string(value: &'static str) -> JsonValue {
    JsonValue::String(value.to_owned())
}

fn object<const N: usize>(entries: [(&str, JsonValue); N]) -> JsonValue {
    JsonValue::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}
