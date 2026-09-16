use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use rand::RngExt;

use crate::graph::{CompiledGraph, NodeKind};
use crate::json_parsing::JsonValue;

use super::convergence::Convergences;
use super::error::{ExecutionFailure, GateSelectionContext, RoutingFailure};
use super::explanation::{ConvergenceReasonInput, Explanation, VisitReason};
use super::gate::select_case;
use super::mapping::apply_mappings;
use super::random::SharedRng;

struct Token {
    node: usize,
    arriving_from: Option<usize>,
    value: JsonValue,
    flows: Vec<String>,
    gate: Option<GateSelectionContext>,
    reason: Option<VisitReason>,
    object_id: Option<usize>,
}

struct PendingTask {
    node: usize,
    task_name: String,
    next: Option<usize>,
    flows: Vec<String>,
    visit_index: Option<usize>,
}

#[derive(Debug)]
pub struct TaskRequest {
    pub ticket: u64,
    pub node: usize,
    pub task_name: String,
    pub payload: JsonValue,
}

#[derive(Debug)]
pub struct Progress {
    pub requests: Vec<TaskRequest>,
    pub output: Option<JsonValue>,
    pub finished: bool,
}

#[derive(Clone)]
pub struct PendingTaskContext {
    pub node: usize,
    pub task_name: String,
}

pub struct Execution {
    graph: Arc<CompiledGraph>,
    rng: SharedRng,
    queue: VecDeque<Token>,
    pending: HashMap<u64, PendingTask>,
    convergences: Convergences,
    next_ticket: u64,
    output: Option<JsonValue>,
    explanation: Option<Explanation>,
    started: bool,
}

impl Execution {
    pub fn new(graph: Arc<CompiledGraph>, rng: SharedRng, payload: JsonValue) -> Self {
        Self::build(graph, rng, payload, false)
    }

    pub fn explained(graph: Arc<CompiledGraph>, rng: SharedRng, payload: JsonValue) -> Self {
        Self::build(graph, rng, payload, true)
    }

    fn build(
        graph: Arc<CompiledGraph>,
        rng: SharedRng,
        payload: JsonValue,
        explained: bool,
    ) -> Self {
        let entry = graph.entry;
        let explanation = explained.then(|| Explanation::new(&payload));
        Self {
            graph,
            rng,
            queue: VecDeque::from([Token {
                node: entry,
                arriving_from: None,
                value: payload,
                flows: Vec::new(),
                gate: None,
                reason: explained.then_some(VisitReason::Entry),
                object_id: explained.then_some(0),
            }]),
            pending: HashMap::new(),
            convergences: Convergences::default(),
            next_ticket: 0,
            output: None,
            explanation,
            started: false,
        }
    }

    pub fn start(&mut self) -> Result<Progress, ExecutionFailure> {
        if self.started {
            return Err(ExecutionFailure::AlreadyStarted);
        }
        self.started = true;
        self.advance()
    }

    pub fn resume(&mut self, ticket: u64, value: JsonValue) -> Result<Progress, ExecutionFailure> {
        let Some(pending) = self.pending.remove(&ticket) else {
            return Err(ExecutionFailure::UnknownTicket);
        };
        let object_id = self.record_object(&value);
        if let (Some(explanation), Some(visit_index), Some(object_id)) =
            (self.explanation.as_mut(), pending.visit_index, object_id)
        {
            explanation.complete_visit(visit_index, object_id);
        }
        self.continue_after(pending.node, pending.next, value, pending.flows, object_id);
        self.advance()
    }

    pub fn take_explanation(&mut self) -> Option<JsonValue> {
        if !self.finished() {
            return None;
        }
        self.explanation.take().and_then(Explanation::into_json)
    }

    pub fn pending_task(&self, ticket: u64) -> Option<PendingTaskContext> {
        self.pending.get(&ticket).map(|pending| PendingTaskContext {
            node: pending.node,
            task_name: pending.task_name.clone(),
        })
    }

    fn advance(&mut self) -> Result<Progress, ExecutionFailure> {
        let mut requests = Vec::new();
        while let Some(token) = self.queue.pop_front() {
            self.process_token(token, &mut requests)?;
            if self.finished() {
                break;
            }
        }
        Ok(Progress {
            requests,
            output: self.output.clone(),
            finished: self.finished(),
        })
    }

    fn process_token(
        &mut self,
        token: Token,
        requests: &mut Vec<TaskRequest>,
    ) -> Result<(), ExecutionFailure> {
        let node_index = token.node;
        let node = self.graph.nodes[node_index].clone();
        match node.kind {
            NodeKind::Task { task, next } => {
                let ticket = self.next_ticket;
                self.next_ticket += 1;
                let visit_index = self.begin_visit(
                    node.id,
                    "task",
                    token.reason,
                    token.object_id.into_iter().collect(),
                );
                self.pending.insert(
                    ticket,
                    PendingTask {
                        node: node_index,
                        task_name: task.clone(),
                        next,
                        flows: token.flows,
                        visit_index,
                    },
                );
                requests.push(TaskRequest {
                    ticket,
                    node: node_index,
                    task_name: task,
                    payload: token.value,
                });
            }
            NodeKind::Map { mappings, next } => {
                let mapped = apply_mappings(&node.id, &token.value, &mappings)?;
                let object_id = self.record_object(&mapped);
                self.record_completed_visit(
                    node.id,
                    "map-schema",
                    token.reason,
                    token.object_id.into_iter().collect(),
                    object_id,
                );
                self.continue_after(node_index, next, mapped, token.flows, object_id);
            }
            NodeKind::Deterministic { select, cases } => {
                let selected_value = token.value.pointer(&select).cloned();
                let (target, case_index, operator) = select_case(selected_value.as_ref(), &cases);
                self.record_completed_visit(
                    node.id.clone(),
                    "deterministic-gate",
                    token.reason,
                    token.object_id.into_iter().collect(),
                    token.object_id,
                );
                let reason = self.explanation.as_ref().map(|_| {
                    VisitReason::deterministic_gate(
                        node.id.clone(),
                        select.as_str().to_owned(),
                        selected_value.clone(),
                        &cases[case_index],
                    )
                });
                self.queue.push_back(Token {
                    node: target,
                    arriving_from: Some(node_index),
                    value: token.value,
                    flows: token.flows,
                    gate: Some(GateSelectionContext {
                        gate_node_id: node.id,
                        selected_path: Some(select.as_str().to_owned()),
                        value_found: Some(selected_value.is_some()),
                        selected_value,
                        matched_operator: Some(operator.to_owned()),
                    }),
                    reason,
                    object_id: token.object_id,
                });
            }
            NodeKind::Randomised { routes, total } => {
                if !total.is_finite()
                    || total <= 0.0
                    || routes
                        .iter()
                        .any(|route| !route.weight.is_finite() || route.weight < 0.0)
                {
                    return Err(ExecutionFailure::InvalidCompiledGraph {
                        node_id: node.id,
                        reason: "randomised distribution contains invalid weights",
                    });
                }
                let Some((fallback_index, _)) = routes
                    .iter()
                    .enumerate()
                    .rfind(|(_, route)| route.weight > 0.0)
                else {
                    return Err(ExecutionFailure::InvalidCompiledGraph {
                        node_id: node.id,
                        reason: "randomised distribution has no positive route",
                    });
                };
                let mut sample = self
                    .rng
                    .lock()
                    .expect("random generator lock poisoned")
                    .random_range(0.0..total);
                let mut selected_index = fallback_index;
                for (index, route) in routes.iter().enumerate() {
                    if sample < route.weight {
                        selected_index = index;
                        break;
                    }
                    sample -= route.weight;
                }
                let selected = &routes[selected_index];
                self.record_completed_visit(
                    node.id.clone(),
                    "randomised-gate",
                    token.reason,
                    token.object_id.into_iter().collect(),
                    token.object_id,
                );
                let reason = self
                    .explanation
                    .as_ref()
                    .map(|_| VisitReason::RandomisedGate {
                        from_node: node.id.clone(),
                        probability: selected.weight / total,
                    });
                self.queue.push_back(Token {
                    node: selected.target,
                    arriving_from: Some(node_index),
                    value: token.value,
                    flows: token.flows,
                    gate: Some(GateSelectionContext {
                        gate_node_id: node.id,
                        selected_path: None,
                        value_found: None,
                        selected_value: None,
                        matched_operator: None,
                    }),
                    reason,
                    object_id: token.object_id,
                });
            }
            NodeKind::FanOut { branches } => {
                self.record_completed_visit(
                    node.id.clone(),
                    "fan-out",
                    token.reason,
                    token.object_id.into_iter().collect(),
                    token.object_id,
                );
                for branch in branches {
                    let mut flows = token.flows.clone();
                    flows.push(branch.flow_id.clone());
                    let reason = self.explanation.as_ref().map(|_| VisitReason::FanOut {
                        from_node: node.id.clone(),
                        flow_id: branch.flow_id,
                    });
                    self.queue.push_back(Token {
                        node: branch.target,
                        arriving_from: Some(node_index),
                        value: token.value.clone(),
                        flows,
                        gate: None,
                        reason,
                        object_id: token.object_id,
                    });
                }
            }
            NodeKind::Converge { inputs, next } => {
                let source = token.arriving_from.expect("validated convergence source");
                if let Some(converged) = self.convergences.arrive(
                    node_index,
                    source,
                    token.value,
                    token.flows,
                    token.object_id,
                    &inputs,
                ) {
                    let object_id = self.record_object(&converged.value);
                    let reason = self.explanation.as_ref().map(|_| VisitReason::Convergence {
                        inputs: inputs
                            .iter()
                            .map(|input| ConvergenceReasonInput {
                                flow_id: input.flow_id.clone(),
                                from_node: self.graph.nodes[input.source].id.clone(),
                            })
                            .collect(),
                    });
                    self.record_completed_visit(
                        node.id,
                        "converge",
                        reason,
                        converged.input_object_ids.unwrap_or_default(),
                        object_id,
                    );
                    self.continue_after(
                        node_index,
                        next,
                        converged.value,
                        converged.flows,
                        object_id,
                    );
                }
            }
            NodeKind::Raise { message } => {
                if let Some(current_object_id) = token.object_id {
                    let input_object_ids = vec![current_object_id];
                    let pending_visits = self
                        .pending
                        .values()
                        .filter_map(|pending| pending.visit_index)
                        .collect::<Vec<_>>();
                    let explanation = self
                        .explanation
                        .as_mut()
                        .expect("explained token has explanation state");
                    explanation.record_terminal_visit(
                        node.id.clone(),
                        "raise-error",
                        token.reason.expect("explained token has a visit reason"),
                        input_object_ids,
                    );
                    for visit in pending_visits {
                        explanation.abandon_visit(visit);
                    }
                    explanation.finish_raise_error(node.id, message, current_object_id);
                    self.pending.clear();
                    self.queue.clear();
                    requests.clear();
                    return Ok(());
                }
                return Err(RoutingFailure {
                    node_id: node.id,
                    message,
                    gate: token.gate,
                }
                .into());
            }
        }
        Ok(())
    }

    fn continue_after(
        &mut self,
        source: usize,
        next: Option<usize>,
        value: JsonValue,
        flows: Vec<String>,
        object_id: Option<usize>,
    ) {
        if source == self.graph.output {
            self.output = Some(value);
            if let (Some(explanation), Some(object_id)) = (&mut self.explanation, object_id) {
                explanation.finish_success(object_id);
            }
        } else {
            let reason = self.explanation.as_ref().map(|_| VisitReason::Next {
                from_node: self.graph.nodes[source].id.clone(),
            });
            self.queue.push_back(Token {
                node: next.expect("validated non-output next"),
                arriving_from: Some(source),
                value,
                flows,
                gate: None,
                reason,
                object_id,
            });
        }
    }

    fn begin_visit(
        &mut self,
        node_id: String,
        node_type: &'static str,
        reason: Option<VisitReason>,
        input_object_ids: Vec<usize>,
    ) -> Option<usize> {
        self.explanation.as_mut().map(|explanation| {
            explanation.begin_visit(
                node_id,
                node_type,
                reason.expect("explained token has a visit reason"),
                input_object_ids,
            )
        })
    }

    fn record_completed_visit(
        &mut self,
        node_id: String,
        node_type: &'static str,
        reason: Option<VisitReason>,
        input_object_ids: Vec<usize>,
        output_object_id: Option<usize>,
    ) {
        if let (Some(explanation), Some(output_object_id)) =
            (&mut self.explanation, output_object_id)
        {
            explanation.record_completed_visit(
                node_id,
                node_type,
                reason.expect("explained token has a visit reason"),
                input_object_ids,
                output_object_id,
            );
        }
    }

    fn record_object(&mut self, value: &JsonValue) -> Option<usize> {
        self.explanation
            .as_mut()
            .map(|explanation| explanation.add_object(value))
    }

    fn finished(&self) -> bool {
        self.output.is_some() || self.explanation.as_ref().is_some_and(Explanation::finished)
    }
}
