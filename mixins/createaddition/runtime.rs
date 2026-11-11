// Create Crafts & Additions — the pure FE network rules.
//
// This file is both the crate's library and the module horizon-mixins injects
// as `crate::game::createaddition`, so it has no game access and no I/O. Every
// number below is the jar's own default, read from the 1.7.2 sources
// (`CommonConfig`, `AbstractConnectorBlockEntity`, `IWireNode`,
// `EnergyNetwork`, `ModularAccumulatorBlockEntity`) and recorded in
// `FINDINGS.md`.

/// Small connector transfer, `FE/t` each way (`small_connector_max_input` and
/// `_max_output`).
pub const SMALL_TRANSFER: i32 = 1_000;
/// Large connector transfer, `FE/t` each way (`large_connector_max_input` and
/// `_max_output`).
pub const LARGE_TRANSFER: i32 = 5_000;
/// A small connector's longest wire, in blocks (`small_connector_wire_length`).
pub const SMALL_WIRE_LENGTH: i32 = 16;
/// A large connector's longest wire, in blocks (`large_connector_wire_length`).
pub const LARGE_WIRE_LENGTH: i32 = 32;
/// A small connector has four nodes, a large one six (`getNodeCount`).
pub const SMALL_NODES: usize = 4;
/// See [`SMALL_NODES`].
pub const LARGE_NODES: usize = 6;
/// The ceiling of one wire network's internal buffer
/// (`connector_network_internal_buffer`, its configured minimum too).
pub const NETWORK_BUFFER: i32 = 80_000;
/// One accumulator block's capacity (`accumulator_capacity`).
pub const ACCUMULATOR_CAPACITY: i32 = 2_000_000;
/// An accumulator's transfer each way (`accumulator_max_input` and `_output`).
pub const ACCUMULATOR_TRANSFER: i32 = 5_000;
/// The constant in `EnergyNetwork.getMaxBuff`: `members * (demand * 3 + 10)`.
const BUFFER_PER_MEMBER: i32 = 10;

/// Which of the jar's two connectors a block is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Connector {
  /// `createaddition:connector` — 1,000 FE/t, 16-block wires, 4 nodes.
  Small,
  /// `createaddition:large_connector` — 5,000 FE/t, 32-block wires, 6 nodes.
  Large,
}

impl Connector {
  /// The connector the block id names, or `None` for another block.
  #[must_use]
  pub fn of_block(block: &str) -> Option<Self> {
    match block {
      "createaddition:connector" => Some(Self::Small),
      "createaddition:large_connector" => Some(Self::Large),
      _ => None,
    }
  }

  /// The most one tick pulls out of the mounted block.
  #[must_use]
  pub const fn max_in(self) -> i32 {
    match self {
      Self::Small => SMALL_TRANSFER,
      Self::Large => LARGE_TRANSFER,
    }
  }

  /// The most one tick inserts into the mounted block.
  #[must_use]
  pub const fn max_out(self) -> i32 {
    self.max_in()
  }

  /// The longest wire this connector takes, in whole blocks.
  #[must_use]
  pub const fn wire_length(self) -> i32 {
    match self {
      Self::Small => SMALL_WIRE_LENGTH,
      Self::Large => LARGE_WIRE_LENGTH,
    }
  }

  /// How many wire nodes the connector has.
  #[must_use]
  pub const fn nodes(self) -> usize {
    match self {
      Self::Small => SMALL_NODES,
      Self::Large => LARGE_NODES,
    }
  }
}

/// The wire kinds this slice ships. The jar's `festive` wire has the same rules
/// as copper but is not carried (it draws lamps); it is recorded as a gap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wire {
  /// `createaddition:copper_wire` — a low-current wire.
  Copper,
  /// `createaddition:gold_wire` — a high-current wire.
  Gold,
  /// `createaddition:electrum_wire` — a high-current wire.
  Electrum,
}

impl Wire {
  /// The wire a spool item id names, or `None` for another item.
  #[must_use]
  pub fn of_spool(item: &str) -> Option<Self> {
    match item {
      "createaddition:copper_spool" => Some(Self::Copper),
      "createaddition:gold_spool" => Some(Self::Gold),
      "createaddition:electrum_spool" => Some(Self::Electrum),
      _ => None,
    }
  }

  /// The wire a saved name names, or `None`.
  #[must_use]
  pub fn of_name(name: &str) -> Option<Self> {
    match name {
      "copper" => Some(Self::Copper),
      "gold" => Some(Self::Gold),
      "electrum" => Some(Self::Electrum),
      _ => None,
    }
  }

  /// The wire's saved name.
  #[must_use]
  pub const fn name(self) -> &'static str {
    match self {
      Self::Copper => "copper",
      Self::Gold => "gold",
      Self::Electrum => "electrum",
    }
  }

  /// Whether the wire may join two large connectors. Upstream refuses copper
  /// (and festive) between two large connectors with
  /// `REQUIRES_HIGH_CURRENT` (`IWireNode.connect`).
  #[must_use]
  pub const fn carries_large(self) -> bool {
    match self {
      Self::Copper => false,
      Self::Gold | Self::Electrum => true,
    }
  }
}

/// A connector's mode (`ConnectorMode`): what it does with the block it is
/// mounted on. Upstream's `Passive` variant is unreachable from the wrench's
/// cycle and is not modelled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
  /// A mounted block that neither gives nor takes energy.
  None,
  /// Draws from the mounted block into the wire network.
  Pull,
  /// Feeds the mounted block from the wire network.
  Push,
}

impl Mode {
  /// The mode a saved name names.
  #[must_use]
  pub fn of_name(name: &str) -> Option<Self> {
    match name {
      "none" => Some(Self::None),
      "pull" => Some(Self::Pull),
      "push" => Some(Self::Push),
      _ => None,
    }
  }

  /// The mode's saved name, and its block-state value.
  #[must_use]
  pub const fn name(self) -> &'static str {
    match self {
      Self::None => "none",
      Self::Pull => "pull",
      Self::Push => "push",
    }
  }
}

/// What a connector does when placed on a block, as upstream's
/// `ConnectorMode.test` decides it: a block that can give is pulled from, one
/// that can only take is pushed into, and one that does neither leaves the
/// connector idle.
#[must_use]
pub const fn placed_mode(can_extract: bool, can_receive: bool) -> Mode {
  if can_extract {
    Mode::Pull
  } else if can_receive {
    Mode::Push
  } else {
    Mode::None
  }
}

/// Why a link was refused. The variants are upstream's `WireConnectResult`
/// failures, minus the ones this slice cannot reach (`OBSTRUCTED` needs a path
/// the jar only uses for contraptions).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
  /// The two connectors are too far apart for the shorter of their wires.
  Long,
  /// The connector has no free node left.
  NodesFull,
  /// The two connectors are already linked.
  AlreadyLinked,
  /// A connector cannot be linked to itself.
  SameBlock,
  /// Copper between two large connectors.
  RequiresHighCurrent,
}

/// What `link` answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link {
  /// The wire was laid.
  Linked,
  /// Nothing changed.
  Refused(Refusal),
}

/// One end of a proposed wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct End {
  /// The connector's block position.
  pub pos: [i32; 3],
  /// Which connector the block is.
  pub connector: Connector,
  /// Whether at least one node has no wire in it.
  pub has_free_node: bool,
  /// Whether a wire already runs to the other end.
  pub linked_to_other: bool,
}

/// The squared distance between two block positions, as
/// `BlockPos.distSqr` measures it.
#[must_use]
pub fn distance_squared(a: [i32; 3], b: [i32; 3]) -> i64 {
  let [dx, dy, dz] = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
  i64::from(dx) * i64::from(dx) + i64::from(dy) * i64::from(dy) + i64::from(dz) * i64::from(dz)
}

/// Whether a wire of `wire` may join `a` and `b`, as `IWireNode.connect`
/// answers: the shorter of the two connectors' lengths bounds the distance,
/// each end needs a free node, the same pair may not be linked twice, and a
/// large-to-large link needs a high-current wire.
#[must_use]
pub fn link(a: End, b: End, wire: Wire) -> Link {
  if a.pos == b.pos {
    return Link::Refused(Refusal::SameBlock);
  }
  let reach = a.connector.wire_length().min(b.connector.wire_length());
  if distance_squared(a.pos, b.pos) > i64::from(reach) * i64::from(reach) {
    return Link::Refused(Refusal::Long);
  }
  if a.linked_to_other || b.linked_to_other {
    return Link::Refused(Refusal::AlreadyLinked);
  }
  if !a.has_free_node || !b.has_free_node {
    return Link::Refused(Refusal::NodesFull);
  }
  if a.connector == Connector::Large && b.connector == Connector::Large && !wire.carries_large() {
    return Link::Refused(Refusal::RequiresHighCurrent);
  }
  Link::Linked
}

/// One network's internal buffer, as upstream's `EnergyNetwork` holds it: two
/// halves swapped every tick, so this tick's intake is next tick's supply.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pool {
  /// What this tick may give away (`outBuff`).
  pub ready: i32,
  /// What this tick takes in, for the next tick (`inBuff`).
  pub next: i32,
}

impl Pool {
  /// The energy in the pool, both halves.
  #[must_use]
  pub const fn total(&self) -> i32 {
    self.ready + self.next
  }

  /// Swaps the halves, as `EnergyNetwork.tick` does at the start of a tick:
  /// last tick's intake becomes spendable and what was spendable goes back into
  /// the intake half. Nothing is dropped — energy stays in the network until a
  /// push connector takes it.
  pub fn swap(&mut self) {
    std::mem::swap(&mut self.ready, &mut self.next);
  }

  /// How much more the pool may take this tick, given its ceiling.
  #[must_use]
  pub fn room(&self, capacity: i32) -> i32 {
    (capacity - self.total()).max(0)
  }

  /// Takes `amount` out of the ready half; answers what it could give.
  pub fn take(&mut self, amount: i32) -> i32 {
    let moved = amount.min(self.ready).max(0);
    self.ready -= moved;
    moved
  }

  /// Puts `amount` into the next half; answers what it could take.
  pub fn put(&mut self, amount: i32, capacity: i32) -> i32 {
    let moved = amount.min(self.room(capacity)).max(0);
    self.next += moved;
    moved
  }
}

/// One network's buffer ceiling, as `EnergyNetwork.getMaxBuff` computes it:
/// `members * (outDemand + 2*inDemand + 10)` with the configured 80,000 ceiling.
/// `demand` is the push demand the last tick asked for.
#[must_use]
pub fn buffer_capacity(members: usize, demand: i32) -> i32 {
  if members == 0 {
    return 0;
  }
  let per_member = demand
    .max(0)
    .saturating_mul(3)
    .saturating_add(BUFFER_PER_MEMBER);
  let members = i32::try_from(members).unwrap_or(i32::MAX);
  members.saturating_mul(per_member).min(NETWORK_BUFFER)
}

/// How much a push connector moves into its mounted block this tick: the most
/// it may insert, no more than the block accepts, no more than the network has.
#[must_use]
pub fn push_amount(max_out: i32, sink_space: i32, ready: i32) -> i32 {
  max_out.min(sink_space).min(ready).max(0)
}

/// How much a pull connector draws out of its mounted block this tick: the most
/// it may extract, no more than the source holds, no more than the network has
/// room for.
#[must_use]
pub fn pull_amount(max_in: i32, source: i32, room: i32) -> i32 {
  max_in.min(source).min(room).max(0)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn end(pos: [i32; 3], connector: Connector) -> End {
    End {
      pos,
      connector,
      has_free_node: true,
      linked_to_other: false,
    }
  }

  // --- the jar's rates -----------------------------------------------------

  #[test]
  fn small_connector_moves_a_thousand_each_way() {
    assert_eq!(Connector::Small.max_in(), 1_000);
    assert_eq!(Connector::Small.max_out(), 1_000);
  }

  #[test]
  fn large_connector_moves_five_thousand_each_way() {
    assert_eq!(Connector::Large.max_in(), 5_000);
    assert_eq!(Connector::Large.max_out(), 5_000);
  }

  #[test]
  fn accumulator_holds_two_million_and_moves_five_thousand() {
    assert_eq!(ACCUMULATOR_CAPACITY, 2_000_000);
    assert_eq!(ACCUMULATOR_TRANSFER, 5_000);
  }

  #[test]
  fn the_two_block_ids_are_the_two_connectors_and_nothing_else() {
    assert_eq!(
      Connector::of_block("createaddition:connector"),
      Some(Connector::Small)
    );
    assert_eq!(
      Connector::of_block("createaddition:large_connector"),
      Some(Connector::Large)
    );
    assert_eq!(
      Connector::of_block("createaddition:modular_accumulator"),
      None
    );
  }

  // --- wire length ---------------------------------------------------------

  #[test]
  fn a_small_connectors_wire_reaches_sixteen_blocks_and_no_further() {
    let from = end([0, 0, 0], Connector::Small);
    assert_eq!(
      link(from, end([16, 0, 0], Connector::Small), Wire::Copper),
      Link::Linked
    );
    assert_eq!(
      link(from, end([17, 0, 0], Connector::Small), Wire::Copper),
      Link::Refused(Refusal::Long)
    );
  }

  #[test]
  fn the_shorter_connector_bounds_the_pair() {
    let small = end([0, 0, 0], Connector::Small);
    let large = end([20, 0, 0], Connector::Large);
    assert_eq!(link(small, large, Wire::Gold), Link::Refused(Refusal::Long));
  }

  #[test]
  fn a_large_connectors_wire_reaches_thirty_two_blocks() {
    let from = end([0, 0, 0], Connector::Large);
    assert_eq!(
      link(from, end([32, 0, 0], Connector::Large), Wire::Gold),
      Link::Linked
    );
    assert_eq!(
      link(from, end([33, 0, 0], Connector::Large), Wire::Gold),
      Link::Refused(Refusal::Long)
    );
  }

  #[test]
  fn the_distance_is_measured_diagonally_like_blocks_dist_sqr() {
    // 12/12/12 is 20.8 blocks, past a small connector's 16 even though each
    // axis is short.
    let from = end([0, 0, 0], Connector::Small);
    assert_eq!(
      link(from, end([12, 12, 12], Connector::Small), Wire::Copper),
      Link::Refused(Refusal::Long)
    );
    // 9/9/9 is 15.6 blocks: inside 16.
    assert_eq!(
      link(from, end([9, 9, 9], Connector::Small), Wire::Copper),
      Link::Linked
    );
  }

  // --- the other link rules -----------------------------------------------

  #[test]
  fn a_connector_cannot_link_to_itself() {
    let here = end([3, 4, 5], Connector::Small);
    assert_eq!(
      link(here, here, Wire::Copper),
      Link::Refused(Refusal::SameBlock)
    );
  }

  #[test]
  fn a_pair_links_once() {
    let mut b = end([1, 0, 0], Connector::Small);
    b.linked_to_other = true;
    assert_eq!(
      link(end([0, 0, 0], Connector::Small), b, Wire::Copper),
      Link::Refused(Refusal::AlreadyLinked)
    );
  }

  #[test]
  fn every_node_in_use_refuses_the_wire() {
    let mut a = end([0, 0, 0], Connector::Small);
    a.has_free_node = false;
    assert_eq!(
      link(a, end([1, 0, 0], Connector::Small), Wire::Copper),
      Link::Refused(Refusal::NodesFull)
    );
  }

  #[test]
  fn copper_cannot_join_two_large_connectors_but_gold_and_electrum_can() {
    let a = end([0, 0, 0], Connector::Large);
    let b = end([4, 0, 0], Connector::Large);
    assert_eq!(
      link(a, b, Wire::Copper),
      Link::Refused(Refusal::RequiresHighCurrent)
    );
    assert_eq!(link(a, b, Wire::Gold), Link::Linked);
    assert_eq!(link(a, b, Wire::Electrum), Link::Linked);
  }

  #[test]
  fn copper_joins_a_large_connector_to_a_small_one() {
    assert_eq!(
      link(
        end([0, 0, 0], Connector::Large),
        end([4, 0, 0], Connector::Small),
        Wire::Copper
      ),
      Link::Linked
    );
  }

  #[test]
  fn a_spool_names_its_wire() {
    assert_eq!(
      Wire::of_spool("createaddition:copper_spool"),
      Some(Wire::Copper)
    );
    assert_eq!(Wire::of_spool("createaddition:spool"), None);
    assert_eq!(Wire::of_name("electrum"), Some(Wire::Electrum));
    assert_eq!(Wire::of_name("festive"), None);
  }

  // --- placement mode ------------------------------------------------------

  #[test]
  fn a_block_that_gives_places_the_connector_pulling() {
    // An accumulator both gives and takes; upstream checks extraction first.
    assert_eq!(placed_mode(true, true), Mode::Pull);
  }

  #[test]
  fn a_block_that_only_takes_places_the_connector_pushing() {
    assert_eq!(placed_mode(false, true), Mode::Push);
  }

  #[test]
  fn a_block_with_no_energy_places_an_idle_connector() {
    assert_eq!(placed_mode(false, false), Mode::None);
  }

  // --- the network buffer --------------------------------------------------

  #[test]
  fn the_buffer_is_sized_from_the_last_ticks_demand() {
    // Two members asking for 1,000 FE: 2 * (1000 * 3 + 10).
    assert_eq!(buffer_capacity(2, 1_000), 6_020);
    // No demand yet: two members still leave the constant.
    assert_eq!(buffer_capacity(2, 0), 20);
  }

  #[test]
  fn the_buffer_never_passes_the_configured_ceiling() {
    assert_eq!(buffer_capacity(20, 5_000), NETWORK_BUFFER);
    assert_eq!(buffer_capacity(1, i32::MAX), NETWORK_BUFFER);
  }

  #[test]
  fn an_empty_network_has_no_buffer() {
    assert_eq!(buffer_capacity(0, 1_000), 0);
  }

  // --- one tick's transfer -------------------------------------------------

  #[test]
  fn a_push_stops_at_the_connectors_rate() {
    assert_eq!(push_amount(1_000, 2_000_000, 5_000), 1_000);
  }

  #[test]
  fn a_push_stops_at_what_the_block_accepts() {
    assert_eq!(push_amount(1_000, 300, 5_000), 300);
  }

  #[test]
  fn a_push_stops_at_what_the_network_holds() {
    assert_eq!(push_amount(1_000, 2_000_000, 40), 40);
  }

  #[test]
  fn a_pull_stops_at_the_connectors_rate_and_the_sources_charge() {
    assert_eq!(pull_amount(1_000, 800, 80_000), 800);
    assert_eq!(pull_amount(1_000, 900_000, 80_000), 1_000);
  }

  #[test]
  fn a_pull_stops_at_the_networks_room() {
    assert_eq!(pull_amount(1_000, 900_000, 120), 120);
  }

  #[test]
  fn nothing_moves_through_a_full_or_empty_end() {
    assert_eq!(push_amount(1_000, 0, 5_000), 0);
    assert_eq!(pull_amount(1_000, 0, 5_000), 0);
    assert_eq!(pull_amount(0, 5_000, 80_000), 0);
  }

  // --- the pool ------------------------------------------------------------

  #[test]
  fn a_pool_hands_out_this_ticks_intake_only_next_tick() {
    let mut pool = Pool { ready: 0, next: 0 };
    assert_eq!(pool.put(1_000, NETWORK_BUFFER), 1_000);
    assert_eq!(
      pool.take(1_000),
      0,
      "the intake is not spendable in the tick it arrived"
    );
    let total = pool.total();
    pool.swap();
    assert_eq!(
      pool.total(),
      total,
      "the swap moves energy, it never makes or loses any"
    );
    assert_eq!(pool.take(1_000), 1_000);
  }

  #[test]
  fn energy_nobody_took_this_tick_stays_in_the_pool() {
    // Upstream's `EnergyNetwork.tick` swaps the halves (`inBuff = t`), so a
    // buffer that no push connector drained is still there next tick.
    let mut pool = Pool {
      ready: 500,
      next: 0,
    };
    pool.swap();
    assert_eq!(
      pool.total(),
      500,
      "the swap must not drop the undrained half"
    );
    assert_eq!(pool.ready, 0);
    assert_eq!(pool.next, 500);
    pool.swap();
    assert_eq!(pool.ready, 500, "and it comes back for the next tick");
  }

  #[test]
  fn a_pool_stops_at_its_ceiling() {
    let mut pool = Pool { ready: 90, next: 0 };
    assert_eq!(pool.room(100), 10);
    assert_eq!(pool.put(50, 100), 10);
    assert_eq!(pool.total(), 100);
  }

  #[test]
  fn a_full_tick_conserves_every_unit_of_energy() {
    // A source with 4,000 FE, a sink with room for 2,500, a pool of 2,020:
    // whatever the split, source + sink + pool is the same afterwards.
    let mut source = 4_000;
    let mut sink = 0;
    let capacity = 2_000_000;
    let mut pool = Pool { ready: 0, next: 0 };
    let network = buffer_capacity(2, 1_000);
    for _ in 0..6 {
      pool.swap();
      let pushed = push_amount(1_000, capacity - sink, pool.ready);
      pool.take(pushed);
      sink += pushed;
      let pulled = pull_amount(1_000, source, pool.room(network));
      pool.put(pulled, network);
      source -= pulled;
    }
    assert_eq!(source + sink + pool.total(), 4_000);
    assert_eq!(sink, 4_000, "six ticks of 1,000 FE/t move the whole charge");
  }
}
