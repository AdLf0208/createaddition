// Runs at the head of `native_register_commands` (API 8): `/createaddition`.
fn observe_register_commands(commands: &mut Vec<NativeCommand>) {
    crate::game::createaddition_state::on_register_commands(commands);
}
