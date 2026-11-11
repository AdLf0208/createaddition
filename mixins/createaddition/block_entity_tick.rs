// Runs at the head of `native_block_entity_tick` (API 17): the block entity's
// own tick writes nothing (its writes are reverted in its own tick); the
// transfer runs at `level_tick_post`.
fn observe_block_entity_tick(pos: [i32; 3], entity: &mut ModBlockEntity) {
    let _ = pos;
    crate::game::createaddition_state::on_block_entity_tick(entity);
}
