use std::collections::HashMap;

use indexmap::IndexMap;

use crate::graph::ConvergeInput;
use crate::json_parsing::JsonValue;

struct Arrival {
    value: JsonValue,
    flows: Vec<String>,
    object_id: Option<usize>,
}

pub(super) struct Converged {
    pub value: JsonValue,
    pub flows: Vec<String>,
    pub input_object_ids: Option<Vec<usize>>,
}

#[derive(Default)]
pub(super) struct Convergences {
    by_node: HashMap<usize, Vec<Option<Arrival>>>,
}

impl Convergences {
    pub fn arrive(
        &mut self,
        node: usize,
        source: usize,
        value: JsonValue,
        mut flows: Vec<String>,
        object_id: Option<usize>,
        inputs: &[ConvergeInput],
    ) -> Option<Converged> {
        let input_index = inputs
            .iter()
            .position(|input| input.source == source)
            .expect("validated convergence input");
        if let Some(position) = flows
            .iter()
            .rposition(|flow| flow == &inputs[input_index].flow_id)
        {
            flows.remove(position);
        }

        let arrivals = self
            .by_node
            .entry(node)
            .or_insert_with(|| std::iter::repeat_with(|| None).take(inputs.len()).collect());
        arrivals[input_index] = Some(Arrival {
            value,
            flows,
            object_id,
        });
        if !arrivals.iter().all(Option::is_some) {
            return None;
        }

        let mut values = IndexMap::new();
        let mut common_flows: Option<Vec<String>> = None;
        let mut input_object_ids = Vec::with_capacity(inputs.len());
        for (input, arrival) in inputs.iter().zip(arrivals.iter_mut()) {
            let arrival = arrival.take().expect("all convergence inputs arrived");
            values.insert(input.flow_id.clone(), arrival.value);
            if let Some(object_id) = arrival.object_id {
                input_object_ids.push(object_id);
            }
            common_flows = Some(match common_flows {
                None => arrival.flows,
                Some(existing) => common_prefix(existing, &arrival.flows),
            });
        }
        self.by_node.remove(&node);
        Some(Converged {
            value: JsonValue::Object(values),
            flows: common_flows.unwrap_or_default(),
            input_object_ids: (input_object_ids.len() == inputs.len()).then_some(input_object_ids),
        })
    }
}

fn common_prefix(mut left: Vec<String>, right: &[String]) -> Vec<String> {
    let length = left
        .iter()
        .zip(right)
        .take_while(|(left, right)| left == right)
        .count();
    left.truncate(length);
    left
}
