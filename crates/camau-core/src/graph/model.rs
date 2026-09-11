use crate::json_parsing::{JsonPointer, JsonValue};

#[derive(Clone)]
pub struct CompiledGraph {
    pub entry: usize,
    pub output: usize,
    pub nodes: Vec<Node>,
}

#[derive(Clone)]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
}

#[derive(Clone)]
pub enum NodeKind {
    Task {
        task: String,
        next: Option<usize>,
    },
    Map {
        mappings: Vec<Mapping>,
        next: Option<usize>,
    },
    Deterministic {
        select: JsonPointer,
        cases: Vec<GateCase>,
    },
    Randomised {
        routes: Vec<WeightedRoute>,
        total: f64,
    },
    FanOut {
        branches: Vec<Branch>,
    },
    Converge {
        inputs: Vec<ConvergeInput>,
        next: Option<usize>,
    },
    Raise {
        message: String,
    },
}

#[derive(Clone)]
pub struct Mapping {
    pub target: JsonPointer,
    pub source: Option<JsonPointer>,
    pub default: Option<JsonValue>,
    pub value_type: ValueType,
}

#[derive(Clone, Copy)]
pub enum ValueType {
    Object,
    Array,
    String,
    Number,
    Integer,
    Boolean,
    Null,
}

#[derive(Clone)]
pub enum GateCase {
    Eq {
        value: JsonValue,
        target: usize,
    },
    Lt {
        value: JsonNumber,
        target: usize,
    },
    Le {
        value: JsonNumber,
        target: usize,
    },
    Ge {
        value: JsonNumber,
        target: usize,
    },
    Gt {
        value: JsonNumber,
        target: usize,
    },
    Range {
        lower: JsonNumber,
        upper: JsonNumber,
        target: usize,
    },
    Set {
        values: Vec<String>,
        target: usize,
    },
    Otherwise {
        target: usize,
    },
}

#[derive(Clone, Copy, Debug)]
pub enum JsonNumber {
    Integer(i64),
    Float(f64),
}

#[derive(Clone)]
pub struct WeightedRoute {
    pub weight: f64,
    pub target: usize,
}

#[derive(Clone)]
pub struct Branch {
    pub flow_id: String,
    pub target: usize,
}

#[derive(Clone)]
pub struct ConvergeInput {
    pub flow_id: String,
    pub source: usize,
}
