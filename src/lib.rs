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
    pub fn to_ternary(self) -> i8 {
        match self {
            FlowDirection::Against => -1,
            FlowDirection::Still => 0,
            FlowDirection::With => 1,
        }
    }

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
    pub fn new(value: u8) -> Self {
        CurrentStrength(value)
    }

    pub fn zero() -> Self {
        CurrentStrength(0)
    }

    pub fn max() -> Self {
        CurrentStrength(255)
    }

    pub fn value(&self) -> u8 {
        self.0
    }

    /// Is this current effectively still?
    pub fn is_still(&self) -> bool {
        self.0 == 0
    }

    /// Combine two strengths (additive, capped).
    pub fn combine(&self, other: &CurrentStrength) -> CurrentStrength {
        CurrentStrength(self.0.saturating_add(other.0))
    }

    /// Attenuate by a factor (0.0 to 1.0).
    pub fn attenuate(&self, factor: f64) -> CurrentStrength {
        let v = (self.0 as f64 * factor) as u8;
        CurrentStrength(v)
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
    pub fn new(direction: FlowDirection, strength: CurrentStrength) -> Self {
        Current {
            direction,
            strength,
            label: String::new(),
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn direction(&self) -> FlowDirection {
        self.direction
    }

    pub fn strength(&self) -> &CurrentStrength {
        &self.strength
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    /// Merge two currents. Stronger direction wins; strengths combine.
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
    pub fn new(id: u64) -> Self {
        RoomId(id)
    }

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
    pub fn new() -> Self {
        CurrentMap {
            fields: HashMap::new(),
        }
    }

    /// Set the current for a room.
    pub fn set(&mut self, room: RoomId, current: Current) {
        self.fields.insert(room, current);
    }

    /// Get the current at a room.
    pub fn get(&self, room: RoomId) -> Option<&Current> {
        self.fields.get(&room)
    }

    /// Remove a room from the map.
    pub fn remove(&mut self, room: RoomId) {
        self.fields.remove(&room);
    }

    pub fn room_count(&self) -> usize {
        self.fields.len()
    }

    /// Find all rooms with non-zero current.
    pub fn active_rooms(&self) -> Vec<RoomId> {
        self.fields
            .iter()
            .filter(|(_, c)| !c.strength().is_still())
            .map(|(id, _)| *id)
            .collect()
    }

    /// Find the room with the strongest current.
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
    pub fn new(room: RoomId) -> Self {
        UpstreamSource {
            room,
            output_strength: CurrentStrength::max(),
            connected: true,
        }
    }

    pub fn with_strength(mut self, strength: CurrentStrength) -> Self {
        self.output_strength = strength;
        self
    }

    /// Emit a current from this source.
    pub fn emit(&self, direction: FlowDirection) -> Option<Current> {
        if self.connected {
            Some(Current::new(direction, self.output_strength))
        } else {
            None
        }
    }

    pub fn disconnect(&mut self) {
        self.connected = false;
    }

    pub fn connect(&mut self) {
        self.connected = true;
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

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
    pub fn new(room: RoomId, capacity: usize) -> Self {
        DownstreamConsumer {
            room,
            received: Vec::new(),
            capacity,
        }
    }

    /// Receive a current. Returns false if at capacity.
    pub fn receive(&mut self, current: Current) -> bool {
        if self.received.len() < self.capacity {
            self.received.push(current);
            true
        } else {
            false
        }
    }

    /// Drain all received currents.
    pub fn drain(&mut self) -> Vec<Current> {
        std::mem::take(&mut self.received)
    }

    pub fn room(&self) -> RoomId {
        self.room
    }

    pub fn received_count(&self) -> usize {
        self.received.len()
    }

    /// Total received strength.
    pub fn total_strength(&self) -> CurrentStrength {
        self.received
            .iter()
            .fold(CurrentStrength::zero(), |acc, c| {
                acc.combine(c.strength())
            })
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
    /// Create an eddy from a cycle of rooms.
    pub fn new(rooms: Vec<RoomId>, strength: CurrentStrength) -> Self {
        CurrentEddy {
            rooms,
            strength,
            active: true,
        }
    }

    /// Follow the eddy from a given room index.
    /// Returns the next room in the cycle, or None if inactive.
    pub fn next(&self, from: RoomId) -> Option<RoomId> {
        if !self.active || self.rooms.len() < 2 {
            return None;
        }
        let idx = self.rooms.iter().position(|r| *r == from)?;
        Some(self.rooms[(idx + 1) % self.rooms.len()])
    }

    /// Deactivate the eddy.
    pub fn dissolve(&mut self) {
        self.active = false;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn rooms(&self) -> &[RoomId] {
        &self.rooms
    }

    pub fn strength(&self) -> &CurrentStrength {
        &self.strength
    }

    /// Generate currents for each hop in the eddy.
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
        assert_eq!(FlowDirection::from_ternary(-1), Some(FlowDirection::Against));
        assert_eq!(FlowDirection::from_ternary(2), None);
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
        let a = Current::new(FlowDirection::With, CurrentStrength::new(80));
        let b = Current::new(FlowDirection::Against, CurrentStrength::new(40));
        let merged = a.merge(&b);
        assert_eq!(merged.direction(), FlowDirection::With); // stronger wins
    }

    #[test]
    fn current_map_set_get() {
        let mut map = CurrentMap::new();
        let room = RoomId::new(1);
        map.set(room, Current::new(FlowDirection::With, CurrentStrength::new(100)));
        assert!(map.get(room).is_some());
        assert_eq!(map.room_count(), 1);
    }

    #[test]
    fn current_map_active_rooms() {
        let mut map = CurrentMap::new();
        let r1 = RoomId::new(1);
        let r2 = RoomId::new(2);
        map.set(r1, Current::new(FlowDirection::With, CurrentStrength::new(50)));
        map.set(r2, Current::new(FlowDirection::Still, CurrentStrength::zero()));
        let active = map.active_rooms();
        assert_eq!(active.len(), 1);
        assert!(active.contains(&r1));
    }

    #[test]
    fn current_map_strongest() {
        let mut map = CurrentMap::new();
        let r1 = RoomId::new(1);
        let r2 = RoomId::new(2);
        map.set(r1, Current::new(FlowDirection::With, CurrentStrength::new(30)));
        map.set(r2, Current::new(FlowDirection::Against, CurrentStrength::new(90)));
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
        let src = UpstreamSource::new(RoomId::new(1))
            .with_strength(CurrentStrength::new(42));
        let c = src.emit(FlowDirection::Against).unwrap();
        assert_eq!(c.strength().value(), 42);
    }

    #[test]
    fn downstream_consumer_receive() {
        let mut consumer = DownstreamConsumer::new(RoomId::new(2), 3);
        assert!(consumer.receive(Current::new(FlowDirection::With, CurrentStrength::new(10))));
        assert!(consumer.receive(Current::new(FlowDirection::Against, CurrentStrength::new(20))));
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
        map.set(r, Current::new(FlowDirection::Still, CurrentStrength::new(10)));
        map.remove(r);
        assert_eq!(map.room_count(), 0);
    }
}
