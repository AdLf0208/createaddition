// Runs at the head of `native_block_entity` (API 15): the three blocks of the
// FE slice get a block entity.
fn observe_block_entity(block: &str, entity: &mut Option<(String, Compound)>) {
    crate::game::createaddition_state::on_block_entity(block, entity);
}
