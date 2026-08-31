use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use rand::RngExt;

use crate::graph::{CompiledGraph, NodeKind};
use crate::json_value::JsonValue;

use super::convergence::Convergences;
use super::error::{ExecutionFailure, GateSelectionContext, RoutingFailure};
use super::gate::select_case;
use super::mapping::apply_mappings;
use super::random::SharedRng;

#[derive(Clone)]
struct Token {
    node: usize,
    arriving_from: Option<usize>,
    value: JsonValue,
    flows: Vec<String>,
    gate: Option<GateSelectionContext>,
}

struct PendingTask {
    node: usize,
    task_name: String,
    next: Option<usize>,
    flows: Vec<String>,
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
    started: bool,
}

impl Execution {
    pub fn new(graph: Arc<CompiledGraph>, rng: SharedRng, payload: JsonValue) -> Self {
        let entry = graph.entry;
        Self {
            graph,
            rng,
            queue: VecDeque::from([Token {
                node: entry,
                arriving_from: None,
                value: payload,
                flows: Vec::new(),
                gate: None,
            }]),
            pending: HashMap::new(),
            convergences: Convergences::default(),
            next_ticket: 0,
            output: None,
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
        self.continue_after(pending.node, pending.next, value, pending.flows);
        self.advance()
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
            if self.output.is_some() {
                break;
            }
        }
        Ok(Progress {
            requests,
            output: self.output.clone(),
        })
    }

    fn process_token(
        &mut self,
        token: Token,
        requests: &mut Vec<TaskRequest>,
    ) -> Result<(), ExecutionFailure> {
        let node = self.graph.nodes[token.node].clone();
        match node.kind {
            NodeKind::Task { task, next } => {
                let ticket = self.next_ticket;
                self.next_ticket += 1;
                self.pending.insert(
                    ticket,
                    PendingTask {
                        node: token.node,
                        task_name: task.clone(),
                        next,
                        flows: token.flows,
                    },
                );
                requests.push(TaskRequest {
                    ticket,
                    node: token.node,
                    task_name: task,
                    payload: token.value,
                });
            }
            NodeKind::Map { mappings, next } => {
                let mapped = apply_mappings(&node.id, &token.value, &mappings)?;
                self.continue_after(token.node, next, mapped, token.flows);
            }
            NodeKind::Deterministic { select, cases } => {
                let selected_value = token.value.pointer(&select).cloned();
                let (target, operator) = select_case(selected_value.as_ref(), &cases);
                self.queue.push_back(Token {
                    node: target,
                    arriving_from: Some(token.node),
                    value: token.value,
                    flows: token.flows,
                    gate: Some(GateSelectionContext {
                        gate_node_id: node.id,
                        selected_path: Some(select),
                        value_found: Some(selected_value.is_some()),
                        selected_value,
                        matched_operator: Some(operator.to_owned()),
                    }),
                });
            }
            NodeKind::Randomised { routes, total } => {
                let mut sample = self
                    .rng
                    .lock()
                    .expect("random generator lock poisoned")
                    .random_range(0.0..total);
                let mut target = routes.last().expect("validated random routes").target;
                for route in &routes {
                    if sample < route.weight {
                        target = route.target;
                        break;
                    }
                    sample -= route.weight;
                }
                self.queue.push_back(Token {
                    node: target,
                    arriving_from: Some(token.node),
                    value: token.value,
                    flows: token.flows,
                    gate: Some(GateSelectionContext {
                        gate_node_id: node.id,
                        selected_path: None,
                        value_found: None,
                        selected_value: None,
                        matched_operator: None,
                    }),
                });
            }
            NodeKind::FanOut { branches } => {
                for branch in branches {
                    let mut flows = token.flows.clone();
                    flows.push(branch.flow_id);
                    self.queue.push_back(Token {
                        node: branch.target,
                        arriving_from: Some(token.node),
                        value: token.value.clone(),
                        flows,
                        gate: None,
                    });
                }
            }
            NodeKind::Converge { inputs, next } => {
                let source = token.arriving_from.expect("validated convergence source");
                if let Some(converged) =
                    self.convergences
                        .arrive(token.node, source, token.value, token.flows, &inputs)
                {
                    self.continue_after(token.node, next, converged.value, converged.flows);
                }
            }
            NodeKind::Raise { message } => {
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
    ) {
        if source == self.graph.output {
            self.output = Some(value);
        } else {
            self.queue.push_back(Token {
                node: next.expect("validated non-output next"),
                arriving_from: Some(source),
                value,
                flows,
                gate: None,
            });
        }
    }
}
