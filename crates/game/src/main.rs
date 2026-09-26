fn window_conf() -> macroquad::prelude::Conf {
    fracture_client::window_conf(fracture_client::Application::Game)
}

#[macroquad::main(window_conf)]
async fn main() {
    fracture_client::run(fracture_client::Application::Game).await;
}
