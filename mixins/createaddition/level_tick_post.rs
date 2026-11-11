// Runs at the head of `native_level_tick_post` (API 23): the whole FE transfer
// of the level, after its block entities ticked.
fn observe_level_tick_post(game: &mut crate::game::Game<'_>, dimension: &str) {
    crate::game::createaddition_state::on_level_tick_post(game, dimension);
}
