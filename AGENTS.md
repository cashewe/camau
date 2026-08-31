# Agents.md

## setup

- use cargo and rust as the primary tool to build.
- use maturin to build code into python compatible objects
- use the latest tool versions to avoid CVEs
- all changes must be kept small and targetted. if a change surface cannot be limited within a module, ask for clarification on the implementation, as this may indicate a poor archtiecture that should not be doubled down on


## code style

- only comment where absolutely neccessary to explain a decision, with a short single line in line comment. no block comment spam
- all run-time sensitive tasks must be built in rust, all interfaces must be python objects
- modules should be deep and boundaries should be narrow.
- a new file per object is a general rule - though if that would result in a file <10 lines, it is ok to merge.
- in rust, do not use the anyhow crate. make use of `thiserror` to produce module scoped errors.
- in python, do not add logging statements. These can be manually added by me as neccessary.

## Agent skills

### Issue tracker

Issues and specifications are tracked in GitHub Issues for `cashewe/camau`. See `docs/agents/issue-tracker.md`.

### Domain docs

This is a single-context repository. See `docs/agents/domain.md`.
