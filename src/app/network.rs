use std::{
    env,
    sync::mpsc,
    thread::{self, JoinHandle},
};

use anyhow::{anyhow, Context};
use chrono::Utc;
use eframe::egui::Context as EguiCtx;
use serde::Serialize;
use tokio::{
    io::AsyncWriteExt,
    select,
    sync::{broadcast, mpsc as ampsc, oneshot},
    task as atask,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

mod server;
mod ws_client;

pub struct Network {
    join_handle: JoinHandle<()>,

    err_rx: mpsc::Receiver<anyhow::Error>,
    err_server_rx: mpsc::Receiver<anyhow::Error>,
    err_ws_client_rx: mpsc::Receiver<anyhow::Error>,

    ws_msg_recv_rx: mpsc::Receiver<String>,
    ws_msg_send_tx: broadcast::Sender<String>,

    stop_token: CancellationToken,

    ctrl_tx: ampsc::UnboundedSender<NetworkCmd>,
    log_tx: ampsc::UnboundedSender<LogEntry>,
}

impl Network {
    pub fn new(egui_ctx: EguiCtx) -> Self {
        info!("initializing network");
        let (err_tx, err_rx) = mpsc::channel();
        let (err_server_tx, err_server_rx) = mpsc::channel();
        let (err_ws_client_tx, err_ws_client_rx) = mpsc::channel();

        let (ws_msg_recv_tx, ws_msg_recv_rx) = mpsc::channel();
        let (ws_msg_send_tx, _) = broadcast::channel::<String>(114514);

        let stop_token = CancellationToken::new();
        let (ctrl_tx, mut ctrl_rx) = ampsc::unbounded_channel();
        let (log_tx, mut log_rx) = ampsc::unbounded_channel();

        let stop_token_cloned = stop_token.clone();
        let egui_ctx_cloned = egui_ctx.clone();
        let ws_msg_send_tx_cloned = ws_msg_send_tx.clone();
        let network_fut = async move {
            let (mut server_stop_token, server_fut) =
                server::run_server(ws_msg_send_tx_cloned.clone());
            let mut server_handle = atask::spawn(server_fut);
            let (mut ws_client_stop_token, ws_client_fut) =
                ws_client::run_ws_client(
                    ws_msg_recv_tx.clone(),
                    egui_ctx_cloned.clone(),
                );
            let mut ws_client_handle = atask::spawn(ws_client_fut);

            let log_file_path = env::current_dir()
                .context("failed to get current working directory")?
                .join("log.jsonl");
            let mut log_file = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_file_path)
                .await
                .context("failed to open log file")?;

            // NOTE: tuple due to rustfmt will mess with args formatting
            let handle_task_result = |(name, result, err_tx): (
                &'static str,
                Result<anyhow::Result<()>, atask::JoinError>,
                Option<mpsc::Sender<anyhow::Error>>,
            )| {
                let err = match result.with_context(|| {
                    format!("failed to join {name} task")
                }) {
                    Ok(result) => {
                        match result.with_context(|| {
                            format!("{name} task exited with an error")
                        }) {
                            Ok(_) => {
                                info!("{name} exited");
                                Some(anyhow!("{name} exited"))
                            }
                            Err(err) => {
                                error!("{err:?}");
                                Some(err)
                            }
                        }
                    }
                    Err(err) => {
                        error!("{err:?}");
                        Some(err)
                    }
                };
                if let (Some(err_tx), Some(err)) = (err_tx, err) {
                    let _ = err_tx.send(err);
                    egui_ctx_cloned.request_repaint();
                }
            };

            loop {
                select! {
                    _ = stop_token_cloned.cancelled()=> {
                        break;
                    }
                    cmd = ctrl_rx.recv() => {
                        let Some(cmd) = cmd else {
                            break;
                        };
                        match cmd {
                            NetworkCmd::RestartServer(done_tx) => {
                                info!("restarting server");
                                server_stop_token.cancel();
                                if !server_handle.is_finished() {
                                    info!("waiting previous server to finish");
                                    handle_task_result(("server", server_handle.await, None));
                                }
                                let (tx, fut) = server::run_server(ws_msg_send_tx_cloned.clone());
                                server_stop_token = tx;
                                server_handle = atask::spawn(fut);
                                let _ = done_tx.send(());
                            },
                            NetworkCmd::RestartWsClient(done_tx) => {
                                info!("restarting ws_client");
                                ws_client_stop_token.cancel();
                                if !ws_client_handle.is_finished() {
                                    info!("waiting previous ws_client to finish");
                                    handle_task_result(("ws_client", ws_client_handle.await, None));
                                }
                                let (tx, fut) = ws_client::run_ws_client(ws_msg_recv_tx.clone(), egui_ctx_cloned.clone());
                                ws_client_stop_token = tx;
                                ws_client_handle = atask::spawn(fut);
                                let _ = done_tx.send(());
                            },
                        }
                    }
                    log = log_rx.recv() => {
                        let Some(log) = log else {
                            break;
                        };
                        let log = serde_json::to_string(&log).context("failed to serialize log")?;
                        log_file.write_all(log.as_bytes()).await.context("failed to write log")?;
                        log_file.write_all(b"\n").await.context("failed to write log(\\n)")?;
                        log_file.flush().await.context("failed to flush log")?;
                    }
                    result = &mut server_handle, if !server_handle.is_finished() => {
                        handle_task_result(("server", result, Some(err_server_tx.clone())));
                    }
                    result = &mut ws_client_handle, if !ws_client_handle.is_finished() => {
                        handle_task_result(("ws_client", result, Some(err_ws_client_tx.clone())));
                    }
                };
            }

            server_stop_token.cancel();
            ws_client_stop_token.cancel();
            if !server_handle.is_finished() {
                handle_task_result(("server", server_handle.await, None));
            }
            if !ws_client_handle.is_finished() {
                handle_task_result((
                    "ws_client",
                    ws_client_handle.await,
                    None,
                ));
            }

            anyhow::Result::<()>::Ok(())
        };

        let network_handle = {
            thread::spawn(move || {
                let result = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .context("failed to build tokio runtime")
                    .and_then(|rt| rt.block_on(network_fut));

                if let Err(err) = result {
                    error!("{err:?}");
                    let _ = err_tx.send(err);
                    egui_ctx.request_repaint();
                };
            })
        };

        Self {
            join_handle: network_handle,

            err_rx,
            err_server_rx,
            err_ws_client_rx,

            ws_msg_recv_rx,
            ws_msg_send_tx,

            stop_token,
            ctrl_tx,
            log_tx,
        }
    }

    pub fn pull_err(&self) -> Option<anyhow::Error> {
        self.err_rx.try_recv().ok()
    }

    pub fn pull_server_err(&self) -> Option<anyhow::Error> {
        self.err_server_rx.try_recv().ok()
    }

    pub fn pull_ws_client_err(&self) -> Option<anyhow::Error> {
        self.err_ws_client_rx.try_recv().ok()
    }

    pub fn pull_ws_message(&self) -> Option<String> {
        self.ws_msg_recv_rx.try_recv().ok()
    }

    pub fn broadcast_ws_message(&self, msg: String) {
        let result = self.ws_msg_send_tx.send(msg);
        if let Err(err) = result {
            debug!("failed to send message to websocket threads: {err}");
        }
    }

    pub fn write_log(&self, msg: String, is_delete: bool) {
        let result = self.log_tx.send(LogEntry {
            msg,
            is_delete,
            ts: Utc::now(),
        });
        if let Err(err) = result {
            error!("failed to write log: {err:?}");
        }
    }

    pub fn restart_server(&self) -> anyhow::Result<()> {
        let (tx, rx) = oneshot::channel();
        self.ctrl_tx
            .send(NetworkCmd::RestartServer(tx))
            .context("failed to send command")?;
        let _ = rx.blocking_recv();
        Ok(())
    }

    pub fn restart_ws_client(&self) -> anyhow::Result<()> {
        let (tx, rx) = oneshot::channel();
        self.ctrl_tx
            .send(NetworkCmd::RestartWsClient(tx))
            .context("failed to send command")?;
        let _ = rx.blocking_recv();
        Ok(())
    }

    pub fn stop(self) {
        self.stop_token.cancel();
        info!("waiting network thread to finish");
        if let Err(err) = self.join_handle.join() {
            error!("network thread panic with: {err:?}");
        }
    }
}

enum NetworkCmd {
    RestartServer(oneshot::Sender<()>),
    RestartWsClient(oneshot::Sender<()>),
}

#[derive(Debug, Serialize)]
struct LogEntry {
    msg: String,
    is_delete: bool,
    ts: chrono::DateTime<Utc>,
}
