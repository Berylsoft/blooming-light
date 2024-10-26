use core::f32;
use std::{collections::VecDeque, ops::Range, time::Instant};

use anyhow::{anyhow, Context};
use eframe::{
    egui::{
        pos2, CentralPanel, Context as EguiCtx, Grid, Id, Rect, RichText,
        ScrollArea, Sense, Window,
    },
    CreationContext,
};
use tracing::info;

use self::network::Network;

mod font;
mod network;

const MSG_TIMEOUT_SECS: f64 = 10.0;

pub struct App {
    network: anyhow::Result<NetworkState>,
    err_messages: Vec<String>,

    message: VecDeque<(String, Instant, bool)>,
    message_waiting: VecDeque<String>,
    pause: bool,
}

impl App {
    pub fn new(cc: &CreationContext) -> Self {
        font::setup_fonts(&cc.egui_ctx);
        // cc.egui_ctx.set_debug_on_hover(true);

        Self {
            network: Ok(NetworkState::new(cc.egui_ctx.clone())),
            err_messages: vec![],

            message: VecDeque::new(),
            message_waiting: VecDeque::new(),
            pause: false,
        }
    }

    fn update_network_err(&mut self, ctx: &EguiCtx) -> bool {
        if let Ok(ref mut network) = self.network {
            network.update_children_errors();

            if let Some(err) = network.pull_err() {
                let mut network =
                    Err(err).context("fatal error in network thread");
                std::mem::swap(&mut self.network, &mut network);
                if let Ok(network) = network {
                    network.stop()
                }
            }
        }

        match self.network {
            Ok(ref mut network) => {
                if let Some(ref err) = network.network_server_err {
                    let msg = format!("{err:?}");

                    Window::new("Embed server error")
                        .collapsible(false)
                        .resizable(false)
                        .show(ctx, |ui| {
                            ui.label(msg);

                            if ui.button("Restart server").clicked() {
                                let result = network.restart_server();
                                if let Err(err) = result {
                                    self.err_messages
                                        .push(format!("{err:?}"));
                                } else {
                                    network.network_server_err = None;
                                }
                            }
                        });
                }

                if let Some(ref err) = network.network_ws_client_err {
                    let msg = format!("{err:?}");

                    Window::new("Embed Websocket client error")
                        .collapsible(false)
                        .resizable(false)
                        .show(ctx, |ui| {
                            ui.label(msg);

                            if ui.button("Restart client").clicked() {
                                let result = network.restart_ws_client();
                                if let Err(err) = result {
                                    self.err_messages
                                        .push(format!("{err:?}"));
                                } else {
                                    network.network_ws_client_err = None;
                                }
                            }
                        });
                }

                false
            }
            Err(ref err) => {
                let msg = format!("{err:?}");

                CentralPanel::default().show(ctx, |ui| {
                    ui.label(msg);
                    if ui.button("Retry").clicked() {
                        self.network = Ok(NetworkState::new(ctx.clone()));
                    }
                });

                true
            }
        }
    }

    fn update_err_messages(&mut self, ctx: &EguiCtx) {
        if !self.err_messages.is_empty() {
            Window::new("Error messages")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    Grid::new("messages")
                        .num_columns(1)
                        .spacing([0.0, 4.0])
                        .striped(true)
                        .min_col_width(ui.available_size_before_wrap().x)
                        .show(ui, |ui| {
                            for msg in &self.err_messages {
                                ui.label(msg);
                                ui.end_row();
                            }
                        });

                    ui.separator();

                    //ui.label(&self.err_messages[0]);
                    //
                    //for msg in &self.err_messages[1..] {
                    //    ui.separator();
                    //    ui.label(msg);
                    //}

                    if ui.button("Clear").clicked() {
                        self.err_messages.clear();
                    }
                });
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &EguiCtx, _frame: &mut eframe::Frame) {
        self.update_err_messages(ctx);

        if self.update_network_err(ctx) {
            return;
        };

        let mut new_msgs = VecDeque::new();
        let Ok(ref network) = self.network else {
            ctx.request_discard("unexpected network err state");
            return;
        };
        while let Some(msg) = network.pull_ws_message() {
            new_msgs.push_back(msg);
        }

        if !self.pause {
            while let Some(msg) = self.message_waiting.pop_front() {
                self.message.push_back((msg, Instant::now(), false));
            }
            while let Some(msg) = new_msgs.pop_front() {
                self.message.push_back((msg, Instant::now(), false));
            }

            while let Some((_, arrive_at, _)) = self.message.front() {
                if arrive_at.elapsed().as_secs_f64() < MSG_TIMEOUT_SECS {
                    break;
                }
                let Some((msg, arrive_at, delete)) =
                    self.message.pop_front()
                else {
                    break;
                };

                assert!(
                    arrive_at.elapsed().as_secs_f64() >= MSG_TIMEOUT_SECS
                );
                assert!(!delete);

                network.broadcast_ws_message(msg);
            }
            self.message.retain(|(_, arrive_at, _)| {
                arrive_at.elapsed().as_secs_f64() < MSG_TIMEOUT_SECS
            });
        } else {
            self.message_waiting.extend(new_msgs);
        }

        CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                if self.pause {
                    ui.label(
                        RichText::new(format!(
                            "Paused, {} message pending",
                            self.message_waiting.len()
                        ))
                        .color(ui.style().visuals.warn_fg_color),
                    );
                } else {
                    ui.label("Receiving");
                }
            });

            ui.separator();

            ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());
                let mut btn_x_range: Range<f32> = f32::INFINITY..0.0;
                let mut btn_press = false;

                for (idx, (msg, arrive_at, delete)) in
                    self.message.iter_mut().rev().enumerate()
                {
                    let mut rect = ui
                        .horizontal(|ui| {
                            let btn_res = ui.button("Delete");
                            let btn_rect = btn_res.rect;
                            btn_x_range.start =
                                btn_x_range.start.min(btn_rect.left());
                            btn_x_range.end =
                                btn_x_range.end.max(btn_rect.right());
                            btn_press |= btn_res
                                .is_pointer_button_down_on()
                                || btn_res.clicked();

                            ui.label(msg.as_str());

                            if btn_res.clicked() {
                                *delete = true;
                            }
                        })
                        .response
                        .rect;

                    // draw bg
                    rect.set_width(ui.available_width());
                    let the_other_row = idx % 2 == 0;
                    if the_other_row {
                        ui.painter().rect_filled(
                            rect,
                            2.0,
                            ui.style().visuals.faint_bg_color,
                        );
                    }

                    // draw timeout progress
                    let progress = (arrive_at.elapsed().as_secs_f64()
                        / MSG_TIMEOUT_SECS)
                        .min(1.0)
                        as f32;
                    rect.set_width(rect.width() * progress);
                    rect = rect.with_min_y(rect.bottom());
                    rect.set_height(ui.spacing().item_spacing.y);
                    ui.painter().rect_filled(
                        rect,
                        1.0,
                        ui.style()
                            .visuals
                            .warn_fg_color
                            .gamma_multiply(0.4),
                    );
                    if progress < 1.0 {
                        ui.ctx().request_repaint();
                    }
                }

                self.message.retain(|(_, _, delete)| !delete);

                let btn_area = Id::new("message list button area");
                let hovered = ui
                    .interact(
                        Rect::from_min_max(
                            pos2(btn_x_range.start, ui.clip_rect().top()),
                            pos2(
                                btn_x_range.end,
                                ui.clip_rect().bottom(),
                            ),
                        ),
                        btn_area,
                        Sense::hover(),
                    )
                    .hovered();

                self.pause = hovered || btn_press;
            })
        });
    }

    fn on_exit(&mut self) {
        info!("exiting");
        let mut network = Err(anyhow!("stopping network"));
        std::mem::swap(&mut self.network, &mut network);
        if let Ok(network) = network {
            info!("stopping network thread");
            network.stop()
        }
    }
}

struct NetworkState {
    network: Network,
    pub network_server_err: Option<anyhow::Error>,
    pub network_ws_client_err: Option<anyhow::Error>,
}

impl NetworkState {
    pub fn new(egui_ctx: EguiCtx) -> Self {
        Self {
            network: Network::new(egui_ctx),
            network_server_err: None,
            network_ws_client_err: None,
        }
    }

    pub fn update_children_errors(&mut self) {
        if self.network_server_err.is_none() {
            self.network_server_err = self.network.pull_server_err();
        }
        if self.network_ws_client_err.is_none() {
            self.network_ws_client_err =
                self.network.pull_ws_client_err();
        }
    }

    delegate::delegate! {
        to self.network {
            pub fn pull_err(&self) -> Option<anyhow::Error>;
            pub fn pull_ws_message(&self) -> Option<String>;
            pub fn broadcast_ws_message(&self, msg: String);
            pub fn restart_server(&self) -> anyhow::Result<()>;
            pub fn restart_ws_client(&self) -> anyhow::Result<()>;
            pub fn stop(self);
        }
    }
}
