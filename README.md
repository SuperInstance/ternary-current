# ternary-current: Information flow and momentum through fleet topologies

`Current`, `CurrentStrength`, `CurrentMap`, `UpstreamSource`, `DownstreamConsumer`, and `CurrentEddy` — models for how information propagates directionally through a fleet of rooms.

## Why This Exists

In a fleet topology, information doesn't just exist — it flows. A sensor reading originates in one room, propagates through neighbors, and ends up at a consumer. That flow has direction, magnitude, and sometimes it circles back. This crate models those flows as currents: directed, strength-weighted movements of information through rooms. Inspired by Oracle1's Current interconnection layer.

## Core Concepts

- **Current** — A directed flow of information with a strength and optional label. Direction is ternary: `Against` (-1), `Still` (0), `With` (+1).
- **CurrentStrength** — Magnitude of flow, 0-255. Can be combined (additive, saturating) or attenuated by a factor.
- **CurrentMap** — A flow field mapping rooms to their local currents. Find active rooms, the strongest flow, or remove stale entries.
- **UpstreamSource** — Where information originates. Emits currents when connected, goes silent when disconnected.
- **DownstreamConsumer** — Where information ends up. Receives currents up to a capacity, then drops overflow. Can compute aggregate strength.
- **CurrentEddy** — A circular flow pattern where information loops through rooms and returns to the start. Can be dissolved to break the cycle.
- **FlowDirection** — Ternary: `Against` (-1), `Still` (0), `With` (+1).
- **RoomId** — Identifier for a room in the fleet topology.

## Quick Start

```toml
[dependencies]
ternary-current = "0.1"
```

```rust
use ternary_current::{
    Current, CurrentStrength, CurrentMap, FlowDirection,
    UpstreamSource, DownstreamConsumer, CurrentEddy, RoomId,
};

// Create a source and emit a current
let source = UpstreamSource::new(RoomId::new(1))
    .with_strength(CurrentStrength::new(100));
let current = source.emit(FlowDirection::With).unwrap();

// Set up a consumer
let mut consumer = DownstreamConsumer::new(RoomId::new(5), 10);
assert!(consumer.receive(current));
assert_eq!(consumer.received_count(), 1);

// Build a flow field
let mut map = CurrentMap::new();
map.set(RoomId::new(1), Current::new(FlowDirection::With, CurrentStrength::new(80)));
map.set(RoomId::new(2), Current::new(FlowDirection::Against, CurrentStrength::new(40)));
assert_eq!(map.strongest(), Some(RoomId::new(1)));

// Create an eddy (circular flow)
let eddy = CurrentEddy::new(
    vec![RoomId::new(1), RoomId::new(2), RoomId::new(3)],
    CurrentStrength::new(60),
);
assert_eq!(eddy.next(RoomId::new(2)), Some(RoomId::new(3)));
assert_eq!(eddy.next(RoomId::new(3)), Some(RoomId::new(1))); // wraps around
```

## API Overview

| Type | What it is |
|------|-----------|
| `Current` | Directed information flow with strength and label |
| `CurrentStrength` | Flow magnitude (0-255), combinable and attenuable |
| `CurrentMap` | Flow field mapping rooms to their local currents |
| `UpstreamSource` | Information origin that emits currents |
| `DownstreamConsumer` | Information sink that receives currents |
| `CurrentEddy` | Circular flow pattern through a cycle of rooms |
| `FlowDirection` | Ternary direction: Against, Still, With |
| `RoomId` | Identifier for a room in the topology |

## How It Works

Information flows from `UpstreamSource` through the topology to `DownstreamConsumer`. A source emits a `Current` (direction + strength) when connected. That current travels through rooms tracked in a `CurrentMap`. Consumers receive currents up to their capacity, then drop overflow.

`Current` merging follows a simple rule: the stronger current's direction wins, and strengths are summed (saturating at 255). This means two opposing currents with different strengths don't cancel — the stronger dominates.

`CurrentEddy` models circular information flow: a list of rooms where information from room N always flows to room N+1, wrapping around. This captures feedback loops. An eddy can be dissolved to break the cycle.

`CurrentMap` is a `HashMap<RoomId, Current>` with query methods. `active_rooms` filters to rooms with non-zero strength, and `strongest` finds the room with the highest flow.

## Known Limitations

- `CurrentStrength` is u8 (0-255). High-precision flow magnitude is not supported.
- `CurrentMap` is flat — no hierarchical or multi-hop routing. The consuming code must chain sources and consumers.
- `DownstreamConsumer` silently drops overflow. There's no backpressure mechanism.
- `CurrentEddy` requires at least 2 rooms to cycle. A single-room eddy returns `None` from `next`.
- No time dimension — flows are instantaneous. Modeling propagation delay requires external coordination.

## Use Cases

- **Sensor data pipeline**: A sensor room acts as an `UpstreamSource`, readings flow through the fleet, and an analytics room acts as a `DownstreamConsumer`.
- **Feedback loop detection**: A `CurrentEddy` with rooms A→B→C→A indicates information that never escapes — a potential infinite loop to investigate.
- **Flow field analysis**: A `CurrentMap` reveals which rooms have the strongest information flow, helping identify bottlenecks or critical paths.
- **Source isolation**: Disconnecting an `UpstreamSource` stops its emissions without removing it from the topology.

## Ecosystem Context

Part of the SuperInstance ternary fleet library. This is the *information flow* layer. `ternary-helm` (navigation) uses current information to decide steering. `ternary-anchor` (stability) uses flow patterns to find stable positions. `ternary-tidepool` (testing) can simulate currents in a sandboxed environment. Inspired by Oracle1's Current interconnection layer.

## License

MIT
