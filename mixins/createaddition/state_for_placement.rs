// Runs at the head of `native_state_for_placement` (API 76): a connector is
// mounted on the face it was clicked against.
fn observe_state_for_placement(
    level: &crate::game::GameLevel<'_>,
    state: BlockStateId,
    context: &PlacementContext,
    answer: &mut Option<Option<BlockStateId>>,
) {
    crate::game::createaddition_state::on_state_for_placement(level, state, context, answer);
}
