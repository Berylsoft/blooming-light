use std::{future::Future, sync::mpsc::Sender};

use eframe::egui::Context as EguiCtx;
use futures_util::StreamExt;
use tokio::select;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;

pub fn run_ws_client(
    message_tx: Sender<String>,
    egui_ctx: EguiCtx,
) -> (CancellationToken, impl Future<Output = anyhow::Result<()>>) {
    let stop_token = CancellationToken::new();
    let stop_token_cloned = stop_token.clone();

    let fut = async move {
        let (ws_stream, _) = connect_async("ws://127.0.0.1:8082").await?;
        let (_, mut read) = ws_stream.split();

        loop {
            select! {
                msg = read.next() => {
                    let Some(msg) = msg else {
                        break;
                    };
                    let msg = msg?;
                    let Message::Text(msg) = msg else {
                        continue;
                    };
                    let result = message_tx.send(msg);
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
