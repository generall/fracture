fn window_conf() -> macroquad::prelude::Conf {
    fracture_client::ai_arena_window_conf()
}
#[macroquad::main(window_conf)]
async fn main() {
    fracture_client::run_ai_arena().await;
}
