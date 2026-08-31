use std::collections::HashMap;

use indexmap::IndexMap;

use crate::graph::ConvergeInput;
use crate::json_value::JsonValue;

struct Arrival {
    value: JsonValue,
    flows: Vec<String>,
}

pub(super) struct Converged {
    pub value: JsonValue,
    pub flows: Vec<String>,
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
        arrivals[input_index] = Some(Arrival { value, flows });
        if !arrivals.iter().all(Option::is_some) {
            return None;
        }

        let mut values = IndexMap::new();
        let mut common_flows: Option<Vec<String>> = None;
        for (input, arrival) in inputs.iter().zip(arrivals.iter_mut()) {
            let arrival = arrival.take().expect("all convergence inputs arrived");
            values.insert(input.flow_id.clone(), arrival.value);
            common_flows = Some(match common_flows {
                None => arrival.flows,
                Some(existing) => common_prefix(existing, &arrival.flows),
            });
        }
        self.by_node.remove(&node);
        Some(Converged {
            value: JsonValue::Object(values),
            flows: common_flows.unwrap_or_default(),
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
