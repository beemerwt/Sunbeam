# Sunbeam

Sunbeam is an open-source Linux streaming host prototype designed around **global host + per-session agents**.

## Goals

- Keep host process independent from any single desktop session.
- Discover and select available session agents (starting with X11).
- Route capture and input through the selected session agent.
- Keep encoding in the host process.

## Workspace layout

- `crates/sunbeam-common`: shared protocol, frame, input, and session types.
- `crates/sunbeam-host`: global host process scaffold + frame ingest + control socket routing.
- `crates/sunbeam-agent-x11`: per-session X11 agent scaffold + synthetic stream mode + XTest input injection.
- `crates/sunbeam-ctl`: control CLI for sessions + input injection commands.
- `crates/sunbeam-client`: minimal LAN client for RTSP playback + TCP control commands.
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

## Milestone 2 demo (local H.264 preview)

Terminal A:

```bash
cargo run -p sunbeam-host -- \
  --socket-path /tmp/sunbeam.sock \
  --h264-output ./artifacts/preview/session-preview.mp4 \
  --h264-fps 30
```

Terminal B:

```bash
DISPLAY=:1 cargo run -p sunbeam-agent-x11 -- \
  --session-name "Media Desktop" \
  --host-socket /tmp/sunbeam.sock \
  --stream-frames \
  --fps 30 \
  --frame-count 300
```

Preview:

```bash
ffplay ./artifacts/preview/session-preview.mp4
```

## Milestone 3 demo (session selection + input injection)

Sunbeam host now also exposes a control socket at:

- main agent socket: `/tmp/sunbeam.sock`
- control socket: `/tmp/sunbeam.sock.ctl`

Start host and two agents (example displays `:0` and `:1`):

```bash
cargo run -p sunbeam-host -- --socket-path /tmp/sunbeam.sock
DISPLAY=:0 cargo run -p sunbeam-agent-x11 -- --stream-frames --host-socket /tmp/sunbeam.sock --session-name "Desktop 0"
DISPLAY=:1 cargo run -p sunbeam-agent-x11 -- --stream-frames --host-socket /tmp/sunbeam.sock --session-name "Desktop 1"
```

Use `sunbeamctl` to inspect/select sessions and inject input to the active session:

```bash
cargo run -p sunbeam-ctl -- sessions
cargo run -p sunbeam-ctl -- select x11-:1
cargo run -p sunbeam-ctl -- move-mouse 500 500
cargo run -p sunbeam-ctl -- mouse-button 1 press
cargo run -p sunbeam-ctl -- mouse-button 1 release
cargo run -p sunbeam-ctl -- key 38 press
cargo run -p sunbeam-ctl -- key 38 release
```

Notes:

- Input injection currently implements `PointerMoveAbsolute`, `PointerButton`, and `Key` in the X11 agent.
- Relative pointer movement, text, and gamepad input types are currently recognized by protocol but not yet injected by the X11 backend.
- Key events use raw X11 keycodes.

## Milestone 4 demo (minimal LAN remote client)

Milestone 4 adds:

- RTSP stream output from `sunbeam-host` (`--rtsp-port`, `--rtsp-path`).
- TCP control listener in `sunbeam-host` (`--control-port`).
- `sunbeam-client` for simple remote playback + input forwarding.

Start host:

```bash
cargo run -p sunbeam-host -- \
  --socket-path /tmp/sunbeam.sock \
  --control-port 47989 \
  --rtsp-port 8554 \
  --rtsp-path sunbeam
```

Start one agent:

```bash
DISPLAY=:1 cargo run -p sunbeam-agent-x11 -- \
  --session-name "LAN Desktop" \
  --host-socket /tmp/sunbeam.sock \
  --stream-frames
```

From another machine on the same LAN (replace `HOST_IP`):

```bash
# play remote stream (uses ffplay by default)
cargo run -p sunbeam-client -- \
  --host HOST_IP \
  --rtsp-port 8554 \
  --stream-path sunbeam \
  play

# list/select sessions over TCP control
cargo run -p sunbeam-client -- --host HOST_IP sessions
cargo run -p sunbeam-client -- --host HOST_IP select x11-:1

# inject input over TCP control
cargo run -p sunbeam-client -- --host HOST_IP move-mouse 640 360
cargo run -p sunbeam-client -- --host HOST_IP mouse-button 1 press
cargo run -p sunbeam-client -- --host HOST_IP mouse-button 1 release
cargo run -p sunbeam-client -- --host HOST_IP key 38 press
cargo run -p sunbeam-client -- --host HOST_IP key 38 release
```

You can also drive remote control from `sunbeamctl`:

```bash
cargo run -p sunbeam-ctl -- --tcp HOST_IP:47989 sessions
cargo run -p sunbeam-ctl -- --tcp HOST_IP:47989 select x11-:1
```

Quick verification with ffplay:

```bash
ffplay rtsp://HOST_IP:8554/sunbeam
```
