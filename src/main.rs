use eframe::egui::ViewportBuilder;
use tracing::{error, info, level_filters::LevelFilter};
use tracing_subscriber::EnvFilter;

mod app;

fn main() -> eframe::Result {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::WARN.into())
                .from_env_lossy(),
        )
        .init();
    if std::env::var("PUFFIN_PROFILER").is_ok_and(|it| it == "true") {
        start_puffin_server()
    }

    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("Blooming Light")
            .with_inner_size([600.0, 400.0]),
        persist_window: true,
        ..Default::default()
    };

    eframe::run_native(
        "BloomingLight",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}

fn start_puffin_server() {
    puffin::set_scopes_on(true);

    match puffin_http::Server::new("127.0.0.1:8585") {
        Ok(puffin_server) => {
            info!("puffin server listenning at 127.0.0.1:8585");

            // We can store the server if we want, but in this case we just want
            // it to keep running. Dropping it closes the server, so let's not drop it!
            #[allow(clippy::mem_forget)]
            std::mem::forget(puffin_server);
        }
        Err(err) => {
            error!("failed to start puffin server: {err}");
        }
    };
}
