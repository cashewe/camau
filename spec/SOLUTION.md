# Solution

we will build a python package, using rust with maturin to keep it performant. that package will consume a json specification file to produce routing rules. the following behaviours should be included in the initial delivery:

| name | type | description |
|------|------|-------------|
| fan-out | routing | route the current value to all listed tasks simultaneously |
| converge | routing | await outputs from all listed tasks and converge into a single json message |
| deterministic-gate | routing | route to sepcific tasks based on the values in a stated field |
| randomised-gate | routing | route to specific tasks randomly (with probabilitiy sampling) |
| map-schema | transformation | map current json message fields into a new structure |

these do not neccessarily all need to be separate, for instance it might make sense to have a single 'route' method that covers fan-out, deterministic-gate and randomised-gate. 

each element in the json schema will have a reference ID that will allow tasks to point their outputs to it, thus allowing users to chain multiple tasks together to achieve for instance the following behaviours:

- a user randomly calls one of two endpoints in order to A/B test a new change
- a user calls two endpoints simultanesouly, but maps the schema to only return one to the user, creating a shadow deployment for testing
- a user uses a determinisitc gate to pick which backend predictor is used based on a field such as 'type' 
- a user calls many models at once, converges the outcomes, maps them to a new schema and sends them t a third location for an overall outcome to be settled on
