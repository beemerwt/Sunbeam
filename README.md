# Sunbeam

Sunbeam is an open-source Linux streaming host prototype designed around **global host + per-session agents**.

## Goals

- Keep host process independent from any single desktop session.
- Discover and select available session agents (starting with X11).
- Route capture and input through the selected session agent.
- Keep encoding in the host process.

## Workspace layout

- `crates/sunbeam-common`: shared protocol, frame, input, and session types.
- `crates/sunbeam-host`: global host process scaffold.
- `crates/sunbeam-agent-x11`: per-session X11 agent scaffold.
- `crates/sunbeam-ctl`: control CLI scaffold.
- `docs/architecture.md`: architecture and milestone plan.

## Quickstart

```bash
cargo run -p sunbeam-host -- --list-sample-sessions
cargo run -p sunbeam-agent-x11 -- --dump-frame frame.bgra
cargo run -p sunbeam-ctl -- sessions
```
