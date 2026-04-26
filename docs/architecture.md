# Sunbeam Architecture (v0 scaffold)

## Core architecture

Sunbeam is split into:

1. `sunbeam-host` (global controller)
2. Session registry and selection
3. Per-session capture agents (`sunbeam-agent-x11` for v0)
4. Per-session input injection
5. Session-aware audio source selection
6. Host-side encoding and transport

## v0 backend direction

- Start with X11 root capture via MIT-SHM.
- Start with BGRA8888 frame transport.
- Convert in host to encoder formats (NV12/YUV420P).
- Add shared-memory FD passing after basic correctness.

## Milestone status

- **M0**: one-frame dump from X11 agent (`--dump-frame`) ✅
- **M1**: agent registration + raw frame transfer + host screenshot dumps ✅ (synthetic frames)
- **M2**: host H.264 encoding preview path ✅ (ffmpeg/libx264 file output)
- **M3**: input routing to active session agent ⏳
- **M4**: minimal remote client for stream + input ⏳

## M1 transport details (current)

- Unix domain socket between agent and host.
- Length-prefixed JSON packet header.
- Optional frame payload for raw BGRA bytes.
- Host stores registered session metadata and writes periodic PNG snapshots.

## Non-goals (v0)

No Wayland/KMS/NVFBC yet, no multi-client, no full web UI, and no Moonlight protocol compatibility until architecture is validated.
