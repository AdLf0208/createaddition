// Runs at the head of `native_block_entity_ticks` (API 15): our block entities
// tick on the server.
fn observe_block_entity_ticks(block: &str, ticks: &mut bool) {
    crate::game::createaddition_state::on_block_entity_ticks(block, ticks);
}
