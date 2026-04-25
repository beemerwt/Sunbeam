# Sunbeam

Sunbeam is an open-source Linux streaming host prototype designed around **global host + per-session agents**.

## Goals

- Keep host process independent from any single desktop session.
- Discover and select available session agents (starting with X11).
- Route capture and input through the selected session agent.
- Keep encoding in the host process.

## Workspace layout

- `crates/sunbeam-common`: shared protocol, frame, input, and session types.
- `crates/sunbeam-host`: global host process scaffold + milestone 1 frame ingest.
- `crates/sunbeam-agent-x11`: per-session X11 agent scaffold + synthetic stream mode.
- `crates/sunbeam-ctl`: control CLI scaffold.
- `docs/architecture.md`: architecture and milestone plan.

## Milestone 1 demo (local)

Terminal A:

```bash
cargo run -p sunbeam-host -- --socket-path /tmp/sunbeam.sock --screenshot-every-n 10
```

Terminal B:

```bash
DISPLAY=:1 cargo run -p sunbeam-agent-x11 -- \
  --session-name "Media Desktop" \
  --host-socket /tmp/sunbeam.sock \
  --stream-frames \
  --fps 10 \
  --frame-count 120
```

Expected behavior:

- Host prints a session table once agent registers.
- Host receives synthetic BGRA frames from the agent.
- Host periodically writes PNG screenshots under `./artifacts/screenshots`.
