use std::{future::Future, net::SocketAddr, sync::Arc, time::Duration};

use anyhow::Context;
use axum::{
    extract::{
        ws::{self, WebSocket},
        ConnectInfo, State, WebSocketUpgrade,
    },
    response::IntoResponse,
    routing, Router,
};
use tokio::{select, sync::Semaphore};
use tokio_util::sync::CancellationToken;
use tower_http::{timeout::TimeoutLayer, trace::TraceLayer};
use tracing::{error, info};

pub fn run_server(
) -> (CancellationToken, impl Future<Output = anyhow::Result<()>>) {
    let stop_token = CancellationToken::new();
    let stop_token_cloned = stop_token.clone();

    let fut = async move {
        let ws_stop_token = CancellationToken::new();
        let ws_semaphore_capacity =
            (Semaphore::MAX_PERMITS as u128).min(u32::MAX as u128) as u32;
        let ws_semaphore =
            Arc::new(Semaphore::new(ws_semaphore_capacity as usize));

        let router = Router::new()
            .route("/ws", routing::any(ws_handler))
            .layer((
                TraceLayer::new_for_http(),
                TimeoutLayer::new(Duration::from_secs(15)),
            ))
            .with_state(ServerState {
                ws_stop_token: ws_stop_token.clone(),
                ws_semaphore: Arc::clone(&ws_semaphore),
            });

        let tcp_listener =
            tokio::net::TcpListener::bind("127.0.0.1:8081")
                .await
                .context("failed to listen 127.0.0.1:8081")?;

        info!(
            "server listening on {}",
            tcp_listener.local_addr().unwrap()
        );

        axum::serve(
            tcp_listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(stop_token_cloned.cancelled_owned())
        .await
        .context("failed to axum::serve")?;

        ws_stop_token.cancel();
        info!("waitting ws sockets to close");
        let _ = ws_semaphore.acquire_many(ws_semaphore_capacity).await;

        anyhow::Result::<()>::Ok(())
    };

    (stop_token, fut)
}

#[derive(Clone)]
struct ServerState {
    ws_stop_token: CancellationToken,
    ws_semaphore: Arc<Semaphore>,
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<ServerState>,
) -> impl IntoResponse {
    info!("new ws connection from {addr}");

    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: ServerState) {
    let permit = match state.ws_semaphore.acquire().await {
        Ok(permit) => permit,
        Err(_) => {
            error!("semaphore closed, closing socket");
            if let Err(err) = socket.close().await {
                error!("failed to close socket: {err:?}");
            }
            return;
        }
    };

    loop {
        let Some(msg) = (select! {
            _ = state.ws_stop_token.cancelled() => {
                info!("socket closing");
                if let Err(err) = socket.close().await {
                    error!("failed to close socket: {err:?}");
                }
                return;
            }
            msg = socket.recv() => {msg}
        }) else {
            break;
        };

        match msg {
            Ok(msg) => match msg {
                ws::Message::Text(_) | ws::Message::Binary(_) => {
                    socket.send(msg).await.unwrap();
                }
                ws::Message::Ping(vec) => {
                    socket.send(ws::Message::Pong(vec)).await.unwrap();
                }
                ws::Message::Pong(_) => {}
                ws::Message::Close(close_frame) => {
                    info!("ws closed: {close_frame:?}")
                }
            },
            Err(err) => {
                error!("{err}");
            }
        }
    }
    drop(permit);
}
