# cross

`Rust Crossbeam framework to walk a (potentially infinite) graph`

Cross is a work-stealing framework - it is currently only able to run on a single machine but will be able to share work across multiple machines in the future.
It uses Crossbeam under the covers to implement a work-stealing algorithm.

Each worker has access to 3 levels of work queues:
1. Local - its own local task queue
2. Global - a top level, global task queue. New tasks added from external sources go in the global queue
3. Stolen - peek at the queues held by other threads and steal their tasks if the current thread doesn't have work

## Future enhancements:

- [ ] Tests
- [ ] Additional examples/worker types - currently http agent is supported
    - [ ] Image downloader
    - [ ] Purely functional graph crawler
    - [ ] JSON graph crawler
- [ ] Data Sinks:
    - [ ] S3 Sink
    - [x] FS Sink
    - [ ] Webhook Sink
    - [ ] Vector Store Sink
- [ ] Add ability to run across multiple nodes and use gossip to discover them
    - [ ] Ability for each node to advertize their "busyness" or "load" factor
    - [ ] Ability to steal work from remote nodes
    - [ ] Ability to steal work intelligently - from the highest loaded node in the cluster
    - [ ] Ability to shutdown gracefull in network mode - if there are other nodes, push work to them before stopping
- [ ] AI Agent Framework:
    - [ ] Make AI agent calls which can enqueue other calls
    - [ ] Support functions

