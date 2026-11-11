// Runs at the head of `native_use_item_on` (API 13): a spool selects a
// connector and lays a wire, the empty spool takes one out.
fn observe_use_on(
    game: &mut crate::game::Game<'_>,
    stack: &ItemStack,
    pos: [i32; 3],
    face: [i32; 3],
    claimed: &mut bool,
) {
    // The face is not needed: a connector knows its mounting from its state.
    let _ = face;
    crate::game::createaddition_state::on_use_item_on(game, stack, pos, claimed);
}
