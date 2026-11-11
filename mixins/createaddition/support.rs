// Game-side glue for Create Crafts & Additions' FE half (injected into
// `crate::game` as `createaddition_state`; it needs the game, so it is not part
// of the standalone library — only `runtime.rs` is).
//
// Three blocks: the two connectors and the modular accumulator. A connector
// stores its mode (`none`/`pull`/`push`) and its wire links in its block
// entity's saved compound; the accumulator stores its charge in the int
// `energy`, which API 142 exposes as a real `Capabilities.Energy.BLOCK` buffer.
//
// A block entity is out of the world during its own tick, so no connector can
// read a neighbour's buffer there: the whole transfer runs after the level's
// tick (`native_level_tick_post`, API 23). Each pass finds the loaded
// connectors, groups them into wire networks from their saved links, and runs
// upstream's cycle: a **pull** connector draws from the block it is mounted on
// into the network's buffer, a **push** connector feeds its mounted block from
// that buffer. Every move is `native_energy_extract` / `native_energy_insert`
// (API 142), so no energy is created; the network's buffer is booked in the
// saved compound of its lowest-positioned connector (`netReady`/`netNext`),
// with the same one-tick swap upstream's `EnergyNetwork` does.

use std::collections::{BTreeMap, BTreeSet};

use horizon_client::containers::{BlockEntity, ModBlockEntity, ModEnergyBuffer};
use horizon_client::items::ItemStack;
use horizon_client::text::Text;
use horizon_core::geometry::Direction;
use horizon_world::nbt::{Compound, JavaString, Tag};
use horizon_world::registry::BlockStateId;

use crate::game::createaddition::{
  ACCUMULATOR_CAPACITY, ACCUMULATOR_TRANSFER, Connector, End, Link, Mode, Pool, Refusal, Wire,
  buffer_capacity, link as plan_link, placed_mode, pull_amount, push_amount,
};
use crate::game::native_server::{NativeCommand, NativeServerPlayer};

const ACCUMULATOR: &str = "createaddition:modular_accumulator";
/// The empty spool: right-clicking a linked connector with it takes the wire out.
const SPOOL: &str = "createaddition:spool";
/// Where a spool remembers the connector its first click selected.
const PENDING: &str = "minecraft:custom_data";
const NODE: &str = "createaddition";

type Pos = [i32; 3];

fn key(name: &str) -> JavaString {
  JavaString::from(name)
}

fn read_int(nbt: &Compound, name: &str) -> i32 {
  match nbt.get(&key(name)) {
    Some(Tag::Int(value)) => *value,
    _ => 0,
  }
}

fn write_int(nbt: &mut Compound, name: &str, value: i32) {
  nbt.insert(key(name), Tag::Int(value));
}

fn read_name(nbt: &Compound, name: &str) -> Option<String> {
  match nbt.get(&key(name)) {
    Some(Tag::String(value)) => Some(value.to_string_lossy()),
    _ => None,
  }
}

fn write_name(nbt: &mut Compound, name: &str, value: &str) {
  nbt.insert(key(name), Tag::String(JavaString::from(value)));
}

fn connector_of(block: &str) -> Option<Connector> {
  Connector::of_block(block)
}

fn is_ours(block: &str) -> bool {
  connector_of(block).is_some() || block == ACCUMULATOR
}

/// The energy buffer a block declares (`native_block_entity_energy`, API 142).
///
/// A connector's buffer is the window upstream declares
/// (`getCapacity() = min(maxIn, maxOut)`, `maxIn`/`maxOut` per call). The pin's
/// descriptor carries no position, so it cannot be limited to the mounted side
/// as upstream's `getEnergyStorage(direction)` is — recorded in `notes.md`.
/// The accumulator is `accumulator_capacity` FE with a 5,000/t limit either way.
#[allow(private_interfaces)]
pub fn on_block_entity_energy(block: &str, energy: &mut Option<ModEnergyBuffer>) {
  *energy = buffer_of(block);
}

fn buffer_of(block: &str) -> Option<ModEnergyBuffer> {
  let (capacity, max_insert, max_extract) = match connector_of(block) {
    Some(connector) => (
      connector.max_in().min(connector.max_out()),
      connector.max_in(),
      connector.max_out(),
    ),
    None if block == ACCUMULATOR => (
      ACCUMULATOR_CAPACITY,
      ACCUMULATOR_TRANSFER,
      ACCUMULATOR_TRANSFER,
    ),
    None => return None,
  };
  Some(ModEnergyBuffer {
    capacity,
    max_insert,
    max_extract,
    insert_faces: [true; 6],
    extract_faces: [true; 6],
  })
}

/// Our blocks have a block entity (`native_block_entity`, API 15).
#[allow(private_interfaces)]
pub fn on_block_entity(block: &str, entity: &mut Option<(String, Compound)>) {
  if is_ours(block) {
    *entity = Some((block.to_owned(), Compound::new()));
  }
}

/// ... and tick on the server (`native_block_entity_ticks`, API 15).
#[allow(private_interfaces)]
pub fn on_block_entity_ticks(block: &str, ticks: &mut bool) {
  if is_ours(block) {
    *ticks = true;
  }
}

/// The per-block-entity tick (`native_block_entity_tick`, API 17). Nothing is
/// written here: a block entity's own write is reverted inside its tick, and the
/// transfer runs at `level_tick_post`. The hook is declared because the pin
/// only ticks a block entity whose mod asked for it.
#[allow(private_interfaces)]
pub fn on_block_entity_tick(entity: &ModBlockEntity) {
  debug_assert!(is_ours(&entity.id), "the hook only names our blocks");
}

/// The face a connector is mounted on, from its block state's `facing`.
fn facing_of(game: &crate::game::Game<'_>, pos: Pos) -> Option<Direction> {
  let state = game.world.block(pos[0], pos[1], pos[2]);
  let descriptor = game.pack.assets.registry.state(state)?;
  let name = descriptor.properties.get("facing")?;
  match name.as_str() {
    "down" => Some(Direction::Down),
    "up" => Some(Direction::Up),
    "north" => Some(Direction::North),
    "south" => Some(Direction::South),
    "west" => Some(Direction::West),
    "east" => Some(Direction::East),
    _ => None,
  }
}

fn offset(pos: Pos, side: Direction) -> Pos {
  let [dx, dy, dz] = side.normal();
  [pos[0] + dx, pos[1] + dy, pos[2] + dz]
}

/// The block a connector is mounted on, and the side of that block which faces
/// the connector.
fn mounted(game: &crate::game::Game<'_>, pos: Pos) -> Option<(Pos, Direction)> {
  let facing = facing_of(game, pos)?;
  Some((offset(pos, facing), facing.opposite()))
}

fn name_of(side: Direction) -> &'static str {
  match side {
    Direction::Down => "down",
    Direction::Up => "up",
    Direction::North => "north",
    Direction::South => "south",
    Direction::West => "west",
    Direction::East => "east",
  }
}

/// A placed block's state (`state_for_placement`, API 76): a connector's
/// `facing` is the side it is mounted on, as upstream's
/// `getClickedFace().getOpposite()`.
#[allow(private_interfaces)]
pub fn on_state_for_placement(
  level: &crate::game::GameLevel<'_>,
  state: BlockStateId,
  context: &horizon_world::level::PlacementContext,
  answer: &mut Option<Option<BlockStateId>>,
) {
  let Some(descriptor) = level.registry.state(state) else {
    return;
  };
  if connector_of(&descriptor.name).is_none() {
    return;
  }
  let facing = name_of(context.face.opposite());
  if let Some(placed) = level.native_state_with_property(state, "facing", facing) {
    *answer = Some(Some(placed));
  }
}

// --- wires ----------------------------------------------------------------

/// One saved wire: the other connector, the node it sits in, and the wire kind.
#[derive(Clone, Debug, PartialEq, Eq)]
struct SavedWire {
  pos: Pos,
  node: i32,
  wire: Wire,
}

fn saved_wires(nbt: &Compound) -> Vec<SavedWire> {
  let Some(Tag::List(entries)) = nbt.get(&key("links")) else {
    return Vec::new();
  };
  entries
    .iter()
    .filter_map(|tag| {
      let Tag::Compound(entry) = tag else {
        return None;
      };
      let pos = [
        read_int(entry, "x"),
        read_int(entry, "y"),
        read_int(entry, "z"),
      ];
      let wire = read_name(entry, "type").and_then(|name| Wire::of_name(&name))?;
      Some(SavedWire {
        pos,
        node: read_int(entry, "node"),
        wire,
      })
    })
    .collect()
}

fn saved_wire_tag(wire: &SavedWire) -> Tag {
  let mut entry = Compound::new();
  write_int(&mut entry, "x", wire.pos[0]);
  write_int(&mut entry, "y", wire.pos[1]);
  write_int(&mut entry, "z", wire.pos[2]);
  write_int(&mut entry, "node", wire.node);
  write_name(&mut entry, "type", wire.wire.name());
  Tag::Compound(entry)
}

fn store_wires(nbt: &mut Compound, wires: &[SavedWire]) {
  nbt.insert(
    key("links"),
    Tag::List(wires.iter().map(saved_wire_tag).collect()),
  );
}

fn mode_of(nbt: &Compound) -> Option<Mode> {
  read_name(nbt, "mode").and_then(|name| Mode::of_name(&name))
}

/// The first node of a connector that has no wire in it.
fn free_node(nbt: &Compound, connector: Connector) -> Option<i32> {
  let used: BTreeSet<i32> = saved_wires(nbt).iter().map(|wire| wire.node).collect();
  (0..i32::try_from(connector.nodes()).ok()?).find(|node| !used.contains(node))
}

// --- reading the level ----------------------------------------------------

fn entity_at<'a>(game: &'a crate::game::Game<'_>, pos: Pos) -> Option<&'a ModBlockEntity> {
  match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
    Some(BlockEntity::Mod(entity)) => Some(entity),
    _ => None,
  }
}

/// Every one of our block entities in the level, in a stable order.
fn our_blocks(game: &crate::game::Game<'_>) -> Vec<(Pos, String)> {
  let mut blocks: Vec<(Pos, String)> = game
    .world
    .block_entities
    .iter()
    .filter_map(|(pos, entity)| match entity {
      BlockEntity::Mod(entity) if is_ours(&entity.id) => {
        Some(([pos.0, pos.1, pos.2], entity.id.clone()))
      }
      _ => None,
    })
    .collect();
  blocks.sort();
  blocks
}

/// What a connector knows at the start of a pass.
#[derive(Clone, Debug)]
struct Node {
  pos: Pos,
  connector: Connector,
  mode: Mode,
  wires: Vec<SavedWire>,
}

fn node_at(game: &crate::game::Game<'_>, pos: Pos, block: &str) -> Option<Node> {
  let connector = connector_of(block)?;
  let entity = entity_at(game, pos)?;
  Some(Node {
    pos,
    connector,
    mode: mode_of(&entity.nbt).unwrap_or(Mode::None),
    wires: saved_wires(&entity.nbt),
  })
}

// --- the level pass -------------------------------------------------------

/// After a level's tick (`native_level_tick_post`, API 23): settle each
/// connector's placement mode, let the accumulators feed the blocks above and
/// below, then run every wire network of the level.
#[allow(private_interfaces)]
pub fn on_level_tick_post(game: &mut crate::game::Game<'_>, dimension: &str) {
  let blocks = our_blocks(game);
  if blocks.is_empty() {
    return;
  }
  for (pos, block) in &blocks {
    if connector_of(block).is_some() {
      settle_mode(game, *pos);
    }
  }
  for (pos, block) in &blocks {
    if block == ACCUMULATOR {
      accumulator_output(game, dimension, *pos);
    }
  }
  let nodes: Vec<Node> = blocks
    .iter()
    .filter_map(|(pos, block)| node_at(game, *pos, block))
    .collect();
  if nodes.len() < 2 {
    return;
  }
  for network in networks(&nodes) {
    if network.len() >= 2 {
      run_network(game, dimension, &network);
    }
  }
}

/// Give a connector its placement mode on the first tick after it is placed, as
/// upstream's `ConnectorMode.test` does at placement: a mounted block that can
/// give is pulled from, one that can only take is pushed into. The mode is kept
/// in the compound and shown in the block's `mode` property.
fn settle_mode(game: &mut crate::game::Game<'_>, pos: Pos) {
  if entity_at(game, pos).is_none() {
    return;
  }
  if entity_at(game, pos).is_some_and(|entity| mode_of(&entity.nbt).is_some()) {
    return;
  }
  let dimension = game.native_dimension();
  let mode = match mounted(game, pos) {
    Some((at, side)) => match game.native_energy(dimension, at, side) {
      Some(info) => placed_mode(info.can_extract, info.can_insert),
      None => Mode::None,
    },
    None => Mode::None,
  };
  let stored = game.native_with_mod_block_entity(pos, |entity| {
    write_name(&mut entity.nbt, "mode", mode.name());
  });
  if stored.is_some() {
    let _ = game.native_set_block_property(pos, "mode", mode.name());
  }
}

/// An accumulator feeds the blocks on its UP and DOWN faces
/// (`ModularAccumulatorBlockEntity.tickOutput`); a single block has both of its
/// `top`/`bottom` state bits set, which this slice always keeps.
fn accumulator_output(game: &mut crate::game::Game<'_>, dimension: &str, pos: Pos) {
  for side in [Direction::Up, Direction::Down] {
    let at = offset(pos, side);
    let Some(info) = game.native_energy(dimension, at, side.opposite()) else {
      continue;
    };
    if !info.can_insert {
      continue;
    }
    let space = info.capacity.saturating_sub(info.amount);
    let max = ACCUMULATOR_TRANSFER.min(space);
    if max <= 0 {
      continue;
    }
    let _ = game.native_move_energy(dimension, pos, side, at, side.opposite(), max);
  }
}

/// The level's connectors grouped into wire networks: two connectors share a
/// network when a wire joins them, directly or through others. Each group is a
/// list of positions in a stable order.
fn networks(nodes: &[Node]) -> Vec<Vec<Pos>> {
  let index: BTreeMap<Pos, usize> = nodes
    .iter()
    .enumerate()
    .map(|(at, node)| (node.pos, at))
    .collect();
  let mut parents: Vec<usize> = (0..nodes.len()).collect();
  fn root(parents: &mut [usize], at: usize) -> usize {
    let mut current = at;
    while parents[current] != current {
      current = parents[current];
    }
    let found = current;
    let mut walk = at;
    while parents[walk] != found {
      let next = parents[walk];
      parents[walk] = found;
      walk = next;
    }
    found
  }
  for (at, node) in nodes.iter().enumerate() {
    for wire in &node.wires {
      if let Some(&other) = index.get(&wire.pos) {
        let (a, b) = (root(&mut parents, at), root(&mut parents, other));
        if a != b {
          parents[a.max(b)] = a.min(b);
        }
      }
    }
  }
  let mut groups: BTreeMap<usize, Vec<Pos>> = BTreeMap::new();
  for at in 0..nodes.len() {
    let group = root(&mut parents, at);
    groups.entry(group).or_default().push(nodes[at].pos);
  }
  groups.into_values().collect()
}

/// One network's tick: swap the buffer, feed every push connector from it, then
/// draw every pull connector into it. The buffer lives in the saved compound of
/// the network's lowest-positioned connector.
fn run_network(game: &mut crate::game::Game<'_>, dimension: &str, members: &[Pos]) {
  let nodes: Vec<Node> = members
    .iter()
    .filter_map(|&pos| {
      let block = entity_at(game, pos)?.id.clone();
      node_at(game, pos, &block)
    })
    .collect();
  if nodes.len() < 2 {
    return;
  }
  let Some(controller) = nodes.iter().map(|node| node.pos).min() else {
    return;
  };
  let demand = entity_at(game, controller).map_or(0, |entity| read_int(&entity.nbt, "netDemand"));
  let capacity = buffer_capacity(nodes.len(), demand);
  let mut pool = Pool {
    ready: entity_at(game, controller).map_or(0, |entity| read_int(&entity.nbt, "netReady")),
    next: entity_at(game, controller).map_or(0, |entity| read_int(&entity.nbt, "netNext")),
  };
  pool.swap();
  let mut wanted = 0;

  for node in &nodes {
    if node.mode != Mode::Push {
      continue;
    }
    let Some((at, side)) = mounted(game, node.pos) else {
      continue;
    };
    let Some(info) = game.native_energy(dimension, at, side) else {
      continue;
    };
    if !info.can_insert {
      continue;
    }
    let space = info.capacity.saturating_sub(info.amount);
    let amount = push_amount(node.connector.max_out(), space, pool.ready);
    if amount == 0 {
      continue;
    }
    wanted += amount;
    let moved = game.native_energy_insert(at, side, amount, false);
    pool.take(moved);
  }

  for node in &nodes {
    if node.mode != Mode::Pull {
      continue;
    }
    let Some((at, side)) = mounted(game, node.pos) else {
      continue;
    };
    let Some(info) = game.native_energy(dimension, at, side) else {
      continue;
    };
    if !info.can_extract {
      continue;
    }
    let amount = pull_amount(node.connector.max_in(), info.amount, pool.room(capacity));
    if amount == 0 {
      continue;
    }
    let moved = game.native_energy_extract(dimension, at, side, amount, false);
    pool.put(moved, capacity);
  }

  let unchanged = entity_at(game, controller).is_some_and(|entity| {
    read_int(&entity.nbt, "netReady") == pool.ready
      && read_int(&entity.nbt, "netNext") == pool.next
      && read_int(&entity.nbt, "netDemand") == wanted
  });
  if unchanged {
    return;
  }
  let _ = game.native_with_mod_block_entity(controller, |entity| {
    write_int(&mut entity.nbt, "netReady", pool.ready);
    write_int(&mut entity.nbt, "netNext", pool.next);
    write_int(&mut entity.nbt, "netDemand", wanted);
  });
}

// --- the spool ------------------------------------------------------------

/// A spool's remembered first click: the connector and its node.
fn pending_of(stack: &ItemStack) -> Option<(Pos, i32)> {
  let root = stack.data().and_then(|data| data.other(PENDING))?;
  let Tag::Compound(root) = root else {
    return None;
  };
  let Tag::Compound(entry) = root.get(&key(NODE))? else {
    return None;
  };
  Some((
    [
      read_int(entry, "x"),
      read_int(entry, "y"),
      read_int(entry, "z"),
    ],
    read_int(entry, "node"),
  ))
}

fn with_pending(stack: &ItemStack, at: Option<(Pos, i32)>) -> ItemStack {
  let mut stack = stack.clone();
  match at {
    Some((pos, node)) => {
      let mut entry = Compound::new();
      write_int(&mut entry, "x", pos[0]);
      write_int(&mut entry, "y", pos[1]);
      write_int(&mut entry, "z", pos[2]);
      write_int(&mut entry, "node", node);
      let mut root = Compound::new();
      root.insert(key(NODE), Tag::Compound(entry));
      stack.data_mut().set_other(PENDING, Some(Tag::Compound(root)));
    }
    None => {
      if let Some(data) = stack.extra.as_deref_mut() {
        data.set_other(PENDING, None);
      }
      stack.tidy();
    }
  }
  stack
}

/// Replace the acting player's held stack inside a use hook (their state is
/// swapped in there).
fn replace_held(game: &mut crate::game::Game<'_>, stack: ItemStack) {
  let slot = game.held_slot();
  *game.held_mut() = Some(stack);
  if game.lan_acting.is_none() {
    game.hand_stack_replaced(slot);
  }
}

fn wire_item(wire: Wire) -> &'static str {
  match wire {
    Wire::Copper => "createaddition:copper_spool",
    Wire::Gold => "createaddition:gold_spool",
    Wire::Electrum => "createaddition:electrum_spool",
  }
}

fn refusal_text(refusal: Refusal) -> &'static str {
  match refusal {
    Refusal::Long => "The connectors are too far apart for this wire.",
    Refusal::NodesFull => "Every node of this connector has a wire.",
    Refusal::AlreadyLinked => "These connectors are already linked.",
    Refusal::SameBlock => "A connector cannot be linked to itself.",
    Refusal::RequiresHighCurrent => "Two large connectors need a gold or electrum wire.",
  }
}

/// The rule check shared by the spool and the command: does `a` and `b` accept
/// `wire`?
fn check(
  game: &crate::game::Game<'_>,
  a: Pos,
  from: Connector,
  b: Pos,
  to: Connector,
  wire: Wire,
) -> Link {
  let from_wires = entity_at(game, a).map(|entity| saved_wires(&entity.nbt)).unwrap_or_default();
  let to_wires = entity_at(game, b).map(|entity| saved_wires(&entity.nbt)).unwrap_or_default();
  let free_from = entity_at(game, a).and_then(|entity| free_node(&entity.nbt, from));
  let free_to = entity_at(game, b).and_then(|entity| free_node(&entity.nbt, to));
  plan_link(
    End {
      pos: a,
      connector: from,
      has_free_node: free_from.is_some(),
      linked_to_other: from_wires.iter().any(|saved| saved.pos == b),
    },
    End {
      pos: b,
      connector: to,
      has_free_node: free_to.is_some(),
      linked_to_other: to_wires.iter().any(|saved| saved.pos == a),
    },
    wire,
  )
}

/// Lay a wire in the two connectors' saved compounds; answers whether both
/// ends took it.
fn store_link(
  game: &mut crate::game::Game<'_>,
  a: Pos,
  from_node: i32,
  b: Pos,
  to: Connector,
  wire: Wire,
) -> bool {
  let Some(to_node) = entity_at(game, b).and_then(|entity| free_node(&entity.nbt, to)) else {
    return false;
  };
  let mut left = entity_at(game, a).map(|entity| saved_wires(&entity.nbt)).unwrap_or_default();
  left.push(SavedWire {
    pos: b,
    node: from_node,
    wire,
  });
  let mut right = entity_at(game, b).map(|entity| saved_wires(&entity.nbt)).unwrap_or_default();
  right.push(SavedWire {
    pos: a,
    node: to_node,
    wire,
  });
  let stored_a = game.native_with_mod_block_entity(a, |entity| {
    store_wires(&mut entity.nbt, &left);
  });
  let stored_b = game.native_with_mod_block_entity(b, |entity| {
    store_wires(&mut entity.nbt, &right);
  });
  stored_a.is_some() && stored_b.is_some()
}

/// A right click with a spool on a connector (`use_item_on`, API 13): the first
/// click selects a node, the second lays the wire, and the empty spool takes a
/// wire out — as upstream's `WireSpool.useOn` does.
#[allow(private_interfaces)]
pub fn on_use_item_on(
  game: &mut crate::game::Game<'_>,
  stack: &ItemStack,
  pos: Pos,
  claimed: &mut bool,
) {
  if stack.item.as_ref() == SPOOL {
    remove_wire(game, pos, stack);
    *claimed = true;
    return;
  }
  let Some(wire) = Wire::of_spool(&stack.item) else {
    return;
  };
  let Some(target) = entity_at(game, pos) else {
    return;
  };
  let Some(target_connector) = connector_of(&target.id) else {
    return;
  };
  let Some(first) = pending_of(stack) else {
    let Some(node) = free_node(&target.nbt, target_connector) else {
      game.native_feedback(Text::literal(refusal_text(Refusal::NodesFull)));
      *claimed = true;
      return;
    };
    replace_held(game, with_pending(stack, Some((pos, node))));
    game.native_feedback(Text::literal("Wire connector selected."));
    *claimed = true;
    return;
  };
  let (from, selected_node) = first;
  replace_held(game, with_pending(stack, None));
  *claimed = true;
  if from == pos {
    return;
  }
  let Some(from_entity) = entity_at(game, from) else {
    return;
  };
  let Some(from_connector) = connector_of(&from_entity.id) else {
    return;
  };
  // The node the first click selected, unless another wire took it since.
  let Some(from_node) = entity_at(game, from)
    .and_then(|entity| free_node(&entity.nbt, from_connector))
    .or(Some(selected_node))
  else {
    return;
  };
  let outcome = check(game, from, from_connector, pos, target_connector, wire);
  match outcome {
    Link::Refused(reason) => {
      game.native_feedback(Text::literal(refusal_text(reason)));
    }
    Link::Linked => {
      if !store_link(game, from, from_node, pos, target_connector, wire) {
        game.native_feedback(Text::literal("The wire could not be saved."));
        return;
      }
      if !game.creative {
        let left = stack.count.saturating_sub(1);
        if left > 0 {
          let mut rest = with_pending(stack, None);
          rest.count = left;
          replace_held(game, rest);
        } else {
          replace_held(game, with_pending(&ItemStack::new(SPOOL, 1), None));
        }
        let _ = game.native_give_items(SPOOL, 1);
      }
      game.native_feedback(Text::literal("Nodes linked."));
    }
  }
}

/// The empty spool takes every wire out of the clicked connector, on both ends.
fn remove_wire(game: &mut crate::game::Game<'_>, pos: Pos, stack: &ItemStack) {
  let Some(entity) = entity_at(game, pos) else {
    return;
  };
  if connector_of(&entity.id).is_none() {
    return;
  }
  let wires = saved_wires(&entity.nbt);
  if wires.is_empty() {
    game.native_feedback(Text::literal("This connector has no wire."));
    return;
  }
  for wire in &wires {
    let _ = game.native_with_mod_block_entity(wire.pos, |other| {
      let kept: Vec<SavedWire> = saved_wires(&other.nbt)
        .into_iter()
        .filter(|saved| saved.pos != pos)
        .collect();
      store_wires(&mut other.nbt, &kept);
    });
  }
  let _ = game.native_with_mod_block_entity(pos, |ours| {
    store_wires(&mut ours.nbt, &[]);
  });
  if !game.creative && stack.count > 0 {
    let spools: u32 = wires.iter().map(|wire| u32::from(!wire.wire.name().is_empty())).sum();
    let _ = game.native_give_items(wire_item(wires[0].wire), spools.max(1));
  }
  game.native_feedback(Text::literal("Wire removed."));
}

// --- commands -------------------------------------------------------------

/// `/createaddition` (`register_commands`, API 8): the harness's readout and
/// controls, because the slice has no GUI and Create's wrench is not installed.
#[allow(private_interfaces)]
pub fn on_register_commands(commands: &mut Vec<NativeCommand>) {
  commands.push(NativeCommand::new("createaddition info <pos:block_pos>"));
  commands.push(NativeCommand::new(
    "createaddition energy <pos:block_pos> <amount:integer>",
  ));
  commands.push(NativeCommand::new(
    "createaddition link <pos:block_pos> <pos2:block_pos> <wire:word>",
  ));
  commands.push(NativeCommand::new(
    "createaddition unlink <pos:block_pos> <pos2:block_pos>",
  ));
  commands.push(NativeCommand::new(
    "createaddition mode <pos:block_pos> <mode:word>",
  ));
}

/// A trailing `#tag` word: the harness marks each command so its reply can be
/// told apart from an older, identical one (`reply` appends it). `None` when the
/// player typed no tag.
fn tag_of<'a>(words: &[&'a str]) -> (Option<&'a str>, Vec<&'a str>) {
  match words.last() {
    Some(last) if last.starts_with('#') && last.len() > 1 => {
      (Some(&last[1..]), words[..words.len() - 1].to_vec())
    }
    _ => (None, words.to_vec()),
  }
}

fn reply(tag: Option<&str>, text: String) -> String {
  match tag {
    Some(tag) => format!("{text} [#{tag}]"),
    None => text,
  }
}

/// `block_pos` is one command word here (`1 2 3`), as the e2e types it; the
/// arguments arrive space-split, so a position takes three words.
fn parse_pos(x: &str, y: &str, z: &str) -> Option<Pos> {
  Some([x.parse().ok()?, y.parse().ok()?, z.parse().ok()?])
}

/// A block's line of text: an accumulator's charge, or a connector's mode,
/// buffer, links and network buffer.
fn info_text(game: &crate::game::Game<'_>, pos: Pos) -> String {
  let Some(entity) = entity_at(game, pos) else {
    return "No Create Crafts & Additions block there.".to_owned();
  };
  if entity.id == ACCUMULATOR {
    return format!(
      "Modular Accumulator at {},{},{}: {}/{} FE (t={})",
      pos[0],
      pos[1],
      pos[2],
      read_int(&entity.nbt, "energy").max(0),
      ACCUMULATOR_CAPACITY,
      game.world.game_time
    );
  }
  let Some(connector) = connector_of(&entity.id) else {
    return "No Create Crafts & Additions block there.".to_owned();
  };
  let wires = saved_wires(&entity.nbt);
  let links = wires
    .iter()
    .map(|wire| {
      format!(
        "({},{},{})/{}/{}",
        wire.pos[0],
        wire.pos[1],
        wire.pos[2],
        wire.node,
        wire.wire.name()
      )
    })
    .collect::<Vec<_>>()
    .join(" ");
  format!(
    "{} at {},{},{}: mode {}, buffer {}/{} FE, links [{}], pool {}/{}, demand {}, t={}",
    match connector {
      Connector::Small => "Connector",
      Connector::Large => "Large Connector",
    },
    pos[0],
    pos[1],
    pos[2],
    mode_of(&entity.nbt).unwrap_or(Mode::None).name(),
    read_int(&entity.nbt, "energy").max(0),
    connector.max_in().min(connector.max_out()),
    links,
    read_int(&entity.nbt, "netReady"),
    read_int(&entity.nbt, "netNext"),
    read_int(&entity.nbt, "netDemand"),
    game.world.game_time
  )
}

#[allow(private_interfaces)]
pub fn on_perform_command(
  game: &mut crate::game::Game<'_>,
  _player: &NativeServerPlayer,
  command: &str,
  ran: &mut bool,
) {
  let words: Vec<&str> = command.split_whitespace().collect();
  let (tag, words) = tag_of(&words);
  match words.as_slice() {
    ["createaddition", "info", x, y, z] => {
      *ran = true;
      let Some(pos) = parse_pos(x, y, z) else {
        game.native_feedback(Text::literal("Usage: /createaddition info <x> <y> <z>"));
        return;
      };
      let answer = reply(tag, info_text(game, pos));
      game.native_feedback(Text::literal(answer));
    }
    ["createaddition", "energy", x, y, z, amount] => {
      *ran = true;
      let Some(pos) = parse_pos(x, y, z) else {
        game.native_feedback(Text::literal(
          "Usage: /createaddition energy <x> <y> <z> <amount>",
        ));
        return;
      };
      let Ok(amount) = amount.parse::<i32>() else {
        game.native_feedback(Text::literal("The amount must be a whole number of FE."));
        return;
      };
      let set = game.native_with_mod_block_entity(pos, |entity| {
        let buffer = buffer_of(&entity.id)?;
        let after = amount.clamp(0, buffer.capacity);
        write_int(&mut entity.nbt, "energy", after);
        Some(after)
      });
      match set {
        Some(Some(after)) => {
          let answer = reply(tag, format!("Energy set to {after} FE."));
          game.native_feedback(Text::literal(answer));
        }
        _ => game.native_feedback(Text::literal("No Create Crafts & Additions block there.")),
      }
    }
    ["createaddition", "mode", x, y, z, mode] => {
      *ran = true;
      let Some(pos) = parse_pos(x, y, z) else {
        game.native_feedback(Text::literal(
          "Usage: /createaddition mode <x> <y> <z> <mode>",
        ));
        return;
      };
      let Some(mode) = Mode::of_name(mode) else {
        let answer = reply(tag, "The mode is none, pull or push.".to_owned());
        game.native_feedback(Text::literal(answer));
        return;
      };
      let stored = game.native_with_mod_block_entity(pos, |entity| connector_of(&entity.id).is_some());
      if stored == Some(true) {
        let _ = game.native_with_mod_block_entity(pos, |entity| {
          write_name(&mut entity.nbt, "mode", mode.name());
        });
        let _ = game.native_set_block_property(pos, "mode", mode.name());
        let answer = reply(tag, format!("Mode set to {}.", mode.name()));
        game.native_feedback(Text::literal(answer));
      } else {
        game.native_feedback(Text::literal("No connector there."));
      }
    }
    ["createaddition", "link", ax, ay, az, bx, by, bz, wire] => {
      *ran = true;
      let (Some(a), Some(b)) = (parse_pos(ax, ay, az), parse_pos(bx, by, bz)) else {
        game.native_feedback(Text::literal(
          "Usage: /createaddition link <x> <y> <z> <x2> <y2> <z2> <copper|gold|electrum>",
        ));
        return;
      };
      let Some(wire) = Wire::of_name(wire) else {
        let answer = reply(tag, "The wire is copper, gold or electrum.".to_owned());
        game.native_feedback(Text::literal(answer));
        return;
      };
      let answer = reply(tag, link_blocks(game, a, b, wire));
      game.native_feedback(Text::literal(answer));
    }
    ["createaddition", "unlink", ax, ay, az, bx, by, bz] => {
      *ran = true;
      let (Some(a), Some(b)) = (parse_pos(ax, ay, az), parse_pos(bx, by, bz)) else {
        game.native_feedback(Text::literal(
          "Usage: /createaddition unlink <x> <y> <z> <x2> <y2> <z2>",
        ));
        return;
      };
      let answer = reply(tag, unlink_blocks(game, a, b));
      game.native_feedback(Text::literal(answer));
    }
    _ => {}
  }
}

fn link_blocks(game: &mut crate::game::Game<'_>, a: Pos, b: Pos, wire: Wire) -> String {
  let Some(first) = entity_at(game, a) else {
    return "No connector at the first position.".to_owned();
  };
  let Some(second) = entity_at(game, b) else {
    return "No connector at the second position.".to_owned();
  };
  let (Some(from), Some(to)) = (connector_of(&first.id), connector_of(&second.id)) else {
    return "Both positions must be connectors.".to_owned();
  };
  match check(game, a, from, b, to, wire) {
    Link::Refused(reason) => refusal_text(reason).to_owned(),
    Link::Linked => {
      let Some(from_node) = entity_at(game, a).and_then(|entity| free_node(&entity.nbt, from)) else {
        return refusal_text(Refusal::NodesFull).to_owned();
      };
      if store_link(game, a, from_node, b, to, wire) {
        "Nodes linked.".to_owned()
      } else {
        refusal_text(Refusal::NodesFull).to_owned()
      }
    }
  }
}

fn unlink_blocks(game: &mut crate::game::Game<'_>, a: Pos, b: Pos) -> String {
  let mut removed = false;
  for (here, there) in [(a, b), (b, a)] {
    let before = entity_at(game, here).map(|entity| saved_wires(&entity.nbt));
    let Some(before) = before else {
      continue;
    };
    let kept: Vec<SavedWire> = before.iter().filter(|saved| saved.pos != there).cloned().collect();
    if kept.len() == before.len() {
      continue;
    }
    removed = true;
    let _ = game.native_with_mod_block_entity(here, |entity| {
      store_wires(&mut entity.nbt, &kept);
    });
  }
  if removed {
    "Wire removed.".to_owned()
  } else {
    "These connectors are not linked.".to_owned()
  }
}
