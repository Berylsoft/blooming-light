use std::{future::Future, sync::mpsc::Sender, time::Duration};

use eframe::egui::Context as EguiCtx;
use tokio::{select, time::interval};
use tokio_util::sync::CancellationToken;

pub fn run_ws_client(
    message_tx: Sender<String>,
    egui_ctx: EguiCtx,
) -> (CancellationToken, impl Future<Output = anyhow::Result<()>>) {
    let stop_token = CancellationToken::new();
    let stop_token_cloned = stop_token.clone();

    let fut = async move {
        let mut count = 0;
        let mut interval = interval(Duration::from_millis(1000));

        loop {
            select! {
                _ = interval.tick() => {
                    count += 1;
                    let result = message_tx.send(format!("message {count}"));
                    if result.is_err() {
                        break;
                    }
                    egui_ctx.request_repaint();
                }
                _ = stop_token_cloned.cancelled() => {
                    break;
                }
            }
        }

        Ok(())
    };

    (stop_token, fut)
}
