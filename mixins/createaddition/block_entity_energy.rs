// Runs at the head of `native_block_entity_energy` (API 142): the declared FE
// buffer of a connector or the accumulator.
fn observe_block_entity_energy(block: &str, energy: &mut Option<ModEnergyBuffer>) {
    crate::game::createaddition_state::on_block_entity_energy(block, energy);
}
