#![forbid(unsafe_code)]

//! ternary-current: Information flow and momentum through fleet topologies.
//!
//! Models directional information propagation as currents: flow direction,
//! magnitude, flow fields across rooms, upstream sources, downstream
//! consumers, and circular eddy patterns. Inspired by Oracle1's Current
//! interconnection layer.

use std::collections::HashMap;

/// Ternary flow direction: against (-1), still (0), with (+1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlowDirection {
    Against,
    Still,
    With,
}

impl FlowDirection {
    /// Encode this direction as a signed ternary value
    /// (`Against` => `-1`, `Still` => `0`, `With` => `1`).
    pub fn to_ternary(self) -> i8 {
        match self {
            FlowDirection::Against => -1,
            FlowDirection::Still => 0,
            FlowDirection::With => 1,
        }
    }

    /// Decode a signed ternary value back into a direction.
    ///
    /// Returns `None` for any value that is not `-1`, `0`, or `1` — the
    /// only three valid ternary encodings. This is intentionally strict so
    /// that out-of-contract inputs surface as `None` instead of being
    /// silently coerced to a default direction.
    pub fn from_ternary(v: i8) -> Option<Self> {
        match v {
            -1 => Some(FlowDirection::Against),
            0 => Some(FlowDirection::Still),
            1 => Some(FlowDirection::With),
            _ => None,
        }
    }
}

/// Magnitude of a current (0-255).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurrentStrength(u8);

impl CurrentStrength {
    /// Wrap a raw `u8` magnitude into a typed strength.
    pub fn new(value: u8) -> Self {
        CurrentStrength(value)
    }

    /// The minimum strength: a current that carries no flow (magnitude `0`).
    pub fn zero() -> Self {
        CurrentStrength(0)
    }

    /// The maximum representable strength (magnitude `255`).
    pub fn max() -> Self {
        CurrentStrength(255)
    }

    /// The raw `u8` magnitude in the range `0..=255`.
    pub fn value(&self) -> u8 {
        self.0
    }

    /// A strength of zero carries no information, so the current is
    /// effectively still regardless of its direction.
    pub fn is_still(&self) -> bool {
        self.0 == 0
    }

    /// Add another strength to this one, saturating at `255` rather than
    /// overflowing. Two `With` currents of `200` and `100` combine to `255`,
    /// not a wrapped `44`.
    pub fn combine(&self, other: &CurrentStrength) -> CurrentStrength {
        CurrentStrength(self.0.saturating_add(other.0))
    }

    /// Scale this strength down by `factor`.
    ///
    /// `factor` is clamped to `[0.0, 1.0]` so that attenuation can only ever
    /// *reduce* magnitude: a factor above `1.0` is treated as `1.0` (no
    /// amplification), and a negative or `NaN` factor is treated as `0.0`
    /// (no flow). This is a total operation that never panics.
    pub fn attenuate(&self, factor: f64) -> CurrentStrength {
        let f = if factor.is_nan() {
            0.0
        } else {
            factor.clamp(0.0, 1.0)
        };
        CurrentStrength((self.0 as f64 * f) as u8)
    }
}

/// A directional current of information.
#[derive(Debug, Clone)]
pub struct Current {
    direction: FlowDirection,
    strength: CurrentStrength,
    label: String,
}

impl Current {
    /// Create a current with the given direction and strength and an empty
    /// label.
    pub fn new(direction: FlowDirection, strength: CurrentStrength) -> Self {
        Current {
            direction,
            strength,
            label: String::new(),
        }
    }

    /// Attach a human-readable label to this current (builder-style).
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// The direction this current flows.
    pub fn direction(&self) -> FlowDirection {
        self.direction
    }

    /// The magnitude of this current.
    pub fn strength(&self) -> &CurrentStrength {
        &self.strength
    }

    /// The descriptive label, empty if none was set.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Merge two currents into one.
    ///
    /// The direction of the *stronger* current wins (ties favor `self`), and
    /// the strengths are summed with [`CurrentStrength::combine`] (saturating
    /// at `255`). The label is `self`'s label if present, otherwise `other`'s;
    /// when both are present they are joined with `+`. This models two flows
    /// joining a channel: the dominant direction carries the combined mass.
    pub fn merge(&self, other: &Current) -> Current {
        let direction = if self.strength.value() >= other.strength.value() {
            self.direction
        } else {
            other.direction
        };
        Current {
            direction,
            strength: self.strength.combine(&other.strength),
            label: if self.label.is_empty() {
                other.label.clone()
            } else {
                format!("{}+{}", self.label, other.label)
            },
        }
    }
}

/// A room identifier in the fleet topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoomId(u64);

impl RoomId {
    /// Create a room identifier from a raw `u64`.
    pub fn new(id: u64) -> Self {
        RoomId(id)
    }

    /// The raw `u64` identifier.
    pub fn value(&self) -> u64 {
        self.0
    }
}

/// A flow field mapping rooms to their local currents.
#[derive(Debug, Clone)]
pub struct CurrentMap {
    fields: HashMap<RoomId, Current>,
}

impl CurrentMap {
    /// Create an empty flow field.
    pub fn new() -> Self {
        CurrentMap {
            fields: HashMap::new(),
        }
    }

    /// Record (or overwrite) the current at a room.
    pub fn set(&mut self, room: RoomId, current: Current) {
        self.fields.insert(room, current);
    }

    /// Look up the current at a room, if any.
    pub fn get(&self, room: RoomId) -> Option<&Current> {
        self.fields.get(&room)
    }

    /// Remove a room from the field. No-op if the room is absent.
    pub fn remove(&mut self, room: RoomId) {
        self.fields.remove(&room);
    }

    /// Number of rooms currently recorded in the field.
    pub fn room_count(&self) -> usize {
        self.fields.len()
    }

    /// All rooms whose current has non-zero magnitude (i.e. is not still).
    pub fn active_rooms(&self) -> Vec<RoomId> {
        self.fields
            .iter()
            .filter(|(_, c)| !c.strength().is_still())
            .map(|(id, _)| *id)
            .collect()
    }

    /// The room carrying the highest-magnitude current.
    ///
    /// Returns `None` when the field is empty. On ties the room chosen is
    /// unspecified (the underlying map is unordered), so callers requiring a
    /// deterministic tie-break should sort the candidates themselves.
    pub fn strongest(&self) -> Option<RoomId> {
        self.fields
            .iter()
            .max_by_key(|(_, c)| c.strength().value())
            .map(|(id, _)| *id)
    }
}

impl Default for CurrentMap {
    fn default() -> Self {
        Self::new()
    }
}

/// An upstream source where information originates.
#[derive(Debug, Clone)]
pub struct UpstreamSource {
    room: RoomId,
    output_strength: CurrentStrength,
    connected: bool,
}

impl UpstreamSource {
    /// Create a connected source located at `room` that emits at maximum
    /// (`255`) strength by default.
    pub fn new(room: RoomId) -> Self {
        UpstreamSource {
            room,
            output_strength: CurrentStrength::max(),
            connected: true,
        }
    }

    /// Override the emitted strength (builder-style).
    pub fn with_strength(mut self, strength: CurrentStrength) -> Self {
        self.output_strength = strength;
        self
    }

    /// Emit a current in `direction` using this source's strength.
    ///
    /// Returns `None` when the source is disconnected — a disconnected source
    /// goes silent rather than emitting a zero-strength placeholder.
    pub fn emit(&self, direction: FlowDirection) -> Option<Current> {
        if self.connected {
            Some(Current::new(direction, self.output_strength))
        } else {
            None
        }
    }

    /// Disconnect the source; subsequent [`emit`](Self::emit) calls return `None`.
    pub fn disconnect(&mut self) {
        self.connected = false;
    }

    /// Reconnect a previously disconnected source.
    pub fn connect(&mut self) {
        self.connected = true;
    }

    /// Whether this source currently emits when [`emit`](Self::emit) is called.
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// The room this source is located at.
    pub fn room(&self) -> RoomId {
        self.room
    }
}

/// A downstream consumer where information ends up.
#[derive(Debug, Clone)]
pub struct DownstreamConsumer {
    room: RoomId,
    received: Vec<Current>,
    capacity: usize,
}

impl DownstreamConsumer {
    /// Create a consumer at `room` that holds at most `capacity` received
    /// currents. A `capacity` of `0` accepts nothing.
    pub fn new(room: RoomId, capacity: usize) -> Self {
        DownstreamConsumer {
            room,
            received: Vec::new(),
            capacity,
        }
    }

    /// Buffer a received current.
    ///
    /// Returns `true` if it was stored, or `false` if the consumer is already
    /// at capacity (in which case the current is dropped — there is no
    /// backpressure).
    pub fn receive(&mut self, current: Current) -> bool {
        if self.received.len() < self.capacity {
            self.received.push(current);
            true
        } else {
            false
        }
    }

    /// Remove and return every received current, leaving the consumer empty.
    pub fn drain(&mut self) -> Vec<Current> {
        std::mem::take(&mut self.received)
    }

    /// The room this consumer is located at.
    pub fn room(&self) -> RoomId {
        self.room
    }

    /// How many currents are currently buffered (`0..=capacity`).
    pub fn received_count(&self) -> usize {
        self.received.len()
    }

    /// Sum of the strengths of all buffered currents (saturating at `255`).
    pub fn total_strength(&self) -> CurrentStrength {
        self.received
            .iter()
            .fold(CurrentStrength::zero(), |acc, c| acc.combine(c.strength()))
    }
}

/// A circular flow pattern (eddy) where information loops back.
#[derive(Debug, Clone)]
pub struct CurrentEddy {
    rooms: Vec<RoomId>,
    strength: CurrentStrength,
    active: bool,
}

impl CurrentEddy {
    /// Create an active eddy over the given cycle of rooms.
    ///
    /// An eddy of fewer than two rooms is inert: [`next`](Self::next) will
    /// always return `None` because there is no other room to flow to.
    pub fn new(rooms: Vec<RoomId>, strength: CurrentStrength) -> Self {
        CurrentEddy {
            rooms,
            strength,
            active: true,
        }
    }

    /// Follow the eddy one hop starting from `from`.
    ///
    /// Returns the next room in the cycle (wrapping from the last room back to
    /// the first), or `None` when the eddy is inactive, has fewer than two
    /// rooms, or `from` is not part of the cycle.
    pub fn next(&self, from: RoomId) -> Option<RoomId> {
        if !self.active || self.rooms.len() < 2 {
            return None;
        }
        let idx = self.rooms.iter().position(|r| *r == from)?;
        Some(self.rooms[(idx + 1) % self.rooms.len()])
    }

    /// Break the cycle: afterwards [`next`](Self::next) and
    /// [`generate_currents`](Self::generate_currents) produce nothing.
    pub fn dissolve(&mut self) {
        self.active = false;
    }

    /// Whether this eddy is still circulating.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// The rooms that make up the cycle, in flow order.
    pub fn rooms(&self) -> &[RoomId] {
        &self.rooms
    }

    /// The per-hop strength assigned to currents this eddy generates.
    pub fn strength(&self) -> &CurrentStrength {
        &self.strength
    }

    /// Produce one [`Current`] per room in the cycle (in flow order), each
    /// carrying the eddy's strength in `direction`. Returns an empty `Vec`
    /// when the eddy is inactive.
    pub fn generate_currents(&self, direction: FlowDirection) -> Vec<Current> {
        if !self.active {
            return Vec::new();
        }
        self.rooms
            .iter()
            .map(|_| Current::new(direction, self.strength))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flow_direction_ternary() {
        assert_eq!(FlowDirection::Against.to_ternary(), -1);
        assert_eq!(FlowDirection::Still.to_ternary(), 0);
        assert_eq!(FlowDirection::With.to_ternary(), 1);
    }

    #[test]
    fn flow_direction_from_ternary() {
        // Cover all three valid encodings, not just one.
        assert_eq!(
            FlowDirection::from_ternary(-1),
            Some(FlowDirection::Against)
        );
        assert_eq!(FlowDirection::from_ternary(0), Some(FlowDirection::Still));
        assert_eq!(FlowDirection::from_ternary(1), Some(FlowDirection::With));
        // Out-of-contract values must surface as None rather than be coerced.
        assert_eq!(FlowDirection::from_ternary(2), None);
        assert_eq!(FlowDirection::from_ternary(-2), None);
        assert_eq!(FlowDirection::from_ternary(i8::MIN), None);
        assert_eq!(FlowDirection::from_ternary(i8::MAX), None);
    }

    #[test]
    fn current_strength_combine() {
        let a = CurrentStrength::new(100);
        let b = CurrentStrength::new(200);
        assert_eq!(a.combine(&b).value(), 255); // saturating
    }

    #[test]
    fn current_strength_attenuate() {
        let s = CurrentStrength::new(100);
        let attenuated = s.attenuate(0.5);
        assert_eq!(attenuated.value(), 50);
    }

    #[test]
    fn current_strength_attenuate_clamps_factor() {
        // Before the clamp fix, factor 2.0 silently AMPLIFIED 100 -> 200
        // despite the documented [0.0, 1.0] range. It must now clamp to 1.0.
        let s = CurrentStrength::new(100);
        assert_eq!(s.attenuate(1.0).value(), 100); // identity / upper bound
        assert_eq!(s.attenuate(2.0).value(), 100); // clamped to 1.0, no amplify
        assert_eq!(s.attenuate(0.0).value(), 0); // lower bound
        assert_eq!(s.attenuate(-1.0).value(), 0); // negative -> 0.0
        assert_eq!(s.attenuate(f64::NAN).value(), 0); // NaN -> 0.0, no panic
                                                      // Confirm a partial factor still rounds down the same way after clamp.
        assert_eq!(CurrentStrength::new(255).attenuate(0.5).value(), 127); // 127.5 -> 127
    }

    #[test]
    fn current_strength_zero() {
        assert!(CurrentStrength::zero().is_still());
        assert!(!CurrentStrength::new(1).is_still());
    }

    #[test]
    fn current_creation() {
        let c = Current::new(FlowDirection::With, CurrentStrength::new(50)).with_label("data");
        assert_eq!(c.direction(), FlowDirection::With);
        assert_eq!(c.strength().value(), 50);
        assert_eq!(c.label(), "data");
    }

    #[test]
    fn current_merge() {
        // Hand-derived: a=(With,80), b=(Against,40). 80>=40 so direction=With;
        // strength = combine(80,40) = 80+40 = 120 (no saturation). Label of
        // the stronger (a) is empty, so the result inherits b's label.
        let a = Current::new(FlowDirection::With, CurrentStrength::new(80)).with_label("alpha");
        let b = Current::new(FlowDirection::Against, CurrentStrength::new(40)).with_label("beta");
        let merged = a.merge(&b);
        assert_eq!(merged.direction(), FlowDirection::With); // stronger wins
        assert_eq!(merged.strength().value(), 120); // sum, exact (not loose bound)
        assert_eq!(merged.label(), "alpha+beta"); // both present -> joined
    }

    #[test]
    fn current_merge_weaker_self_loses_direction() {
        // self weaker: other's direction must win. This pins the < branch of
        // merge, which the equal-strength / stronger-self cases never exercise.
        let a = Current::new(FlowDirection::With, CurrentStrength::new(10));
        let b = Current::new(FlowDirection::Against, CurrentStrength::new(90));
        assert_eq!(a.merge(&b).direction(), FlowDirection::Against);
        assert_eq!(a.merge(&b).strength().value(), 100);
    }

    #[test]
    fn current_merge_strength_saturates() {
        // combine at the limit: 200 + 200 must saturate to 255, not wrap to 144.
        let a = Current::new(FlowDirection::With, CurrentStrength::new(200));
        let b = Current::new(FlowDirection::With, CurrentStrength::new(200));
        assert_eq!(a.merge(&b).strength().value(), 255);
    }

    #[test]
    fn current_map_set_get() {
        let mut map = CurrentMap::new();
        let room = RoomId::new(1);
        map.set(
            room,
            Current::new(FlowDirection::With, CurrentStrength::new(100)),
        );
        assert!(map.get(room).is_some());
        assert_eq!(map.room_count(), 1);
    }

    #[test]
    fn current_map_active_rooms() {
        let mut map = CurrentMap::new();
        let r1 = RoomId::new(1);
        let r2 = RoomId::new(2);
        map.set(
            r1,
            Current::new(FlowDirection::With, CurrentStrength::new(50)),
        );
        map.set(
            r2,
            Current::new(FlowDirection::Still, CurrentStrength::zero()),
        );
        let active = map.active_rooms();
        assert_eq!(active.len(), 1);
        assert!(active.contains(&r1));
    }

    #[test]
    fn current_map_strongest() {
        let mut map = CurrentMap::new();
        let r1 = RoomId::new(1);
        let r2 = RoomId::new(2);
        map.set(
            r1,
            Current::new(FlowDirection::With, CurrentStrength::new(30)),
        );
        map.set(
            r2,
            Current::new(FlowDirection::Against, CurrentStrength::new(90)),
        );
        assert_eq!(map.strongest(), Some(r2));
    }

    #[test]
    fn upstream_source_emit() {
        let src = UpstreamSource::new(RoomId::new(1));
        let c = src.emit(FlowDirection::With).unwrap();
        assert_eq!(c.direction(), FlowDirection::With);
        assert_eq!(c.strength().value(), 255);
    }

    #[test]
    fn upstream_source_disconnect() {
        let mut src = UpstreamSource::new(RoomId::new(1));
        src.disconnect();
        assert!(!src.is_connected());
        assert!(src.emit(FlowDirection::With).is_none());
    }

    #[test]
    fn upstream_source_custom_strength() {
        let src = UpstreamSource::new(RoomId::new(1)).with_strength(CurrentStrength::new(42));
        let c = src.emit(FlowDirection::Against).unwrap();
        assert_eq!(c.strength().value(), 42);
    }

    #[test]
    fn downstream_consumer_receive() {
        let mut consumer = DownstreamConsumer::new(RoomId::new(2), 3);
        assert!(consumer.receive(Current::new(FlowDirection::With, CurrentStrength::new(10))));
        assert!(consumer.receive(Current::new(
            FlowDirection::Against,
            CurrentStrength::new(20)
        )));
        assert_eq!(consumer.received_count(), 2);
    }

    #[test]
    fn downstream_consumer_capacity() {
        let mut consumer = DownstreamConsumer::new(RoomId::new(2), 1);
        consumer.receive(Current::new(FlowDirection::With, CurrentStrength::new(10)));
        assert!(!consumer.receive(Current::new(FlowDirection::With, CurrentStrength::new(10))));
    }

    #[test]
    fn downstream_consumer_drain() {
        let mut consumer = DownstreamConsumer::new(RoomId::new(2), 10);
        consumer.receive(Current::new(FlowDirection::With, CurrentStrength::new(50)));
        consumer.receive(Current::new(FlowDirection::With, CurrentStrength::new(50)));
        let drained = consumer.drain();
        assert_eq!(drained.len(), 2);
        assert_eq!(consumer.received_count(), 0);
    }

    #[test]
    fn downstream_total_strength() {
        let mut consumer = DownstreamConsumer::new(RoomId::new(2), 10);
        consumer.receive(Current::new(FlowDirection::With, CurrentStrength::new(100)));
        consumer.receive(Current::new(FlowDirection::With, CurrentStrength::new(50)));
        assert_eq!(consumer.total_strength().value(), 150);
    }

    #[test]
    fn eddy_next() {
        let rooms = vec![RoomId::new(1), RoomId::new(2), RoomId::new(3)];
        let eddy = CurrentEddy::new(rooms, CurrentStrength::new(30));
        assert_eq!(eddy.next(RoomId::new(1)), Some(RoomId::new(2)));
        assert_eq!(eddy.next(RoomId::new(3)), Some(RoomId::new(1))); // wraps
    }

    #[test]
    fn eddy_dissolve() {
        let rooms = vec![RoomId::new(1), RoomId::new(2)];
        let mut eddy = CurrentEddy::new(rooms, CurrentStrength::new(30));
        eddy.dissolve();
        assert!(!eddy.is_active());
        assert!(eddy.next(RoomId::new(1)).is_none());
    }

    #[test]
    fn eddy_generate_currents() {
        let rooms = vec![RoomId::new(1), RoomId::new(2), RoomId::new(3)];
        let eddy = CurrentEddy::new(rooms, CurrentStrength::new(60));
        let currents = eddy.generate_currents(FlowDirection::With);
        assert_eq!(currents.len(), 3);
        for c in &currents {
            assert_eq!(c.strength().value(), 60);
        }
    }

    #[test]
    fn eddy_too_few_rooms() {
        let eddy = CurrentEddy::new(vec![RoomId::new(1)], CurrentStrength::new(30));
        assert!(eddy.next(RoomId::new(1)).is_none());
    }

    #[test]
    fn current_map_remove() {
        let mut map = CurrentMap::new();
        let r = RoomId::new(1);
        map.set(
            r,
            Current::new(FlowDirection::Still, CurrentStrength::new(10)),
        );
        map.remove(r);
        assert_eq!(map.room_count(), 0);
    }

    // ---- Edge-case / boundary coverage ----

    #[test]
    fn current_map_empty_queries() {
        let mut map = CurrentMap::new();
        assert_eq!(map.room_count(), 0);
        assert!(map.active_rooms().is_empty());
        assert_eq!(map.strongest(), None);
        assert!(map.get(RoomId::new(0)).is_none());
        // remove on an empty map must not panic.
        map.remove(RoomId::new(7));
        assert_eq!(map.room_count(), 0);
    }

    #[test]
    fn downstream_consumer_zero_capacity_rejects_all() {
        let mut consumer = DownstreamConsumer::new(RoomId::new(2), 0);
        assert!(!consumer.receive(Current::new(FlowDirection::With, CurrentStrength::new(10))));
        assert_eq!(consumer.received_count(), 0);
        assert_eq!(consumer.total_strength().value(), 0);
    }

    #[test]
    fn downstream_total_strength_saturates() {
        // Three currents of 100 each -> 300 -> saturates at 255, no wrap.
        let mut consumer = DownstreamConsumer::new(RoomId::new(2), 10);
        consumer.receive(Current::new(FlowDirection::With, CurrentStrength::new(100)));
        consumer.receive(Current::new(FlowDirection::With, CurrentStrength::new(100)));
        consumer.receive(Current::new(FlowDirection::With, CurrentStrength::new(100)));
        assert_eq!(consumer.total_strength().value(), 255);
    }

    #[test]
    fn eddy_next_unknown_room_is_none() {
        let eddy = CurrentEddy::new(
            vec![RoomId::new(1), RoomId::new(2)],
            CurrentStrength::new(30),
        );
        // A room not in the cycle yields None (position-based lookup fails).
        assert_eq!(eddy.next(RoomId::new(99)), None);
    }

    #[test]
    fn eddy_empty_rooms_vec_is_inert() {
        let eddy = CurrentEddy::new(vec![], CurrentStrength::new(30));
        assert_eq!(eddy.next(RoomId::new(1)), None); // len() < 2
                                                     // generate_currents on an empty (but active) eddy returns nothing.
        assert!(eddy.generate_currents(FlowDirection::With).is_empty());
        assert!(eddy.is_active()); // still "active", just has no rooms
    }

    #[test]
    fn eddy_generate_currents_silent_when_dissolved() {
        let mut eddy = CurrentEddy::new(
            vec![RoomId::new(1), RoomId::new(2), RoomId::new(3)],
            CurrentStrength::new(60),
        );
        eddy.dissolve();
        assert!(eddy.generate_currents(FlowDirection::With).is_empty());
    }

    #[test]
    fn upstream_source_emit_label_is_empty_by_default() {
        // Default-emitted current has no label until explicitly set.
        let src = UpstreamSource::new(RoomId::new(1));
        let c = src.emit(FlowDirection::With).unwrap();
        assert_eq!(c.label(), "");
    }

    #[test]
    fn current_merge_empty_label_inherits_other() {
        // self has no label, other does -> result takes other's label verbatim.
        let a = Current::new(FlowDirection::With, CurrentStrength::new(80));
        let b = Current::new(FlowDirection::Against, CurrentStrength::new(40)).with_label("only");
        let merged = a.merge(&b);
        assert_eq!(merged.label(), "only");
    }

    #[test]
    fn attenuate_does_not_exceed_original_even_with_large_factor() {
        // Regression guard for the amplification bug: clamping means the
        // attenuated value can never exceed the original strength.
        let original = CurrentStrength::new(137);
        for factor in [2.0, 10.0, 1000.0, f64::INFINITY] {
            assert!(original.attenuate(factor).value() <= original.value());
        }
    }
}
