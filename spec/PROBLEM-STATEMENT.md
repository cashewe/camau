# Problem Statement

you manage a number of backend APIs. These include AI and Machine Learning capabilities. each API is minimal, exposising only single routes such as `/predict` etc...

Your consumers need to be able to call based on desired outcome, but currently need to know minute details about every API. For instance, there will be situations in which multiple APIs need to be hit from a single request, situations where a certain field may indicate the need to use a different model, etc... These routes may also need to change during the liftetime of the service, for instance, new endpoint versions, new models introduced, A/B testing, etc...

To resolve this, *gateway services* will be produced. each service will manage the routing of input messages to the correct outputs. Since there will be multiple gateway services, it is decided that a shared routing package will be developed to create a simple, performant vocabulary for route management. `camau` is that package.