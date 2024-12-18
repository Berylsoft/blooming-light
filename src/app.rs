use core::{f32, f64};
use std::{collections::VecDeque, ops::Range, time::Instant};

use anyhow::{anyhow, Context};
use demo_source::DemoSource;
use eframe::{
    egui::{
        pos2, CentralPanel, Color32, Context as EguiCtx, DragValue, Grid,
        Id, Rect, RichText, ScrollArea, Sense, Window,
    },
    CreationContext,
};
use tracing::info;

use self::network::Network;

mod demo_source;
mod font;
mod network;

pub struct App {
    network: anyhow::Result<NetworkState>,
    err_messages: Vec<String>,

    message: VecDeque<(String, Instant, bool)>,
    message_waiting: VecDeque<String>,

    pause: bool,

    msg_send_delay_secs: f64,
    msg_send_delay_secs_id: Id,

    demo_settings_show: bool,
    demo_settings_show_id: Id,
    demo_enable: bool,
    demo_enable_id: Id,
    demo_interval_secs: f64,
    demo_interval_secs_id: Id,
    demo_source: DemoSource,
}

impl App {
    pub fn new(cc: &CreationContext) -> Self {
        font::setup_fonts(&cc.egui_ctx);
        // cc.egui_ctx.set_debug_on_hover(true);
        let msg_send_delay_secs_id =
            Id::new("config.msg_send_delay_secs");
        let msg_send_delay_secs = cc
            .egui_ctx
            .data_mut(|d| d.get_persisted::<f64>(msg_send_delay_secs_id))
            .unwrap_or(10.0);
        let demo_settings_show_id = Id::new("config.demo_settings_show");
        let demo_settings_show = cc
            .egui_ctx
            .data_mut(|d| d.get_persisted::<bool>(demo_settings_show_id))
            .unwrap_or(false);
        let demo_enable_id = Id::new("config.demo_enable");
        let demo_enable = cc
            .egui_ctx
            .data_mut(|d| d.get_persisted::<bool>(demo_enable_id))
            .unwrap_or(false);
        let demo_interval_secs_id = Id::new("config.demo_interval_secs");
        let demo_interval_secs = cc
            .egui_ctx
            .data_mut(|d| d.get_persisted::<f64>(demo_interval_secs_id))
            .unwrap_or(0.1);

        Self {
            network: Ok(NetworkState::new(cc.egui_ctx.clone())),
            err_messages: vec![],

            message: VecDeque::new(),
            message_waiting: VecDeque::new(),

            pause: false,

            msg_send_delay_secs,
            msg_send_delay_secs_id,

            demo_settings_show,
            demo_settings_show_id,
            demo_enable,
            demo_enable_id,
            demo_interval_secs,
            demo_interval_secs_id,
            demo_source: DemoSource::default(),
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
                    if !self.demo_enable {
                        let msg = format!("{err:?}");

                        Window::new("Embed Websocket client error")
                            .collapsible(false)
                            .resizable(false)
                            .show(ctx, |ui| {
                                ui.label(msg);

                                if ui.button("Restart client").clicked() {
                                    let result =
                                        network.restart_ws_client();
                                    if let Err(err) = result {
                                        self.err_messages
                                            .push(format!("{err:?}"));
                                    } else {
                                        network.network_ws_client_err =
                                            None;
                                    }
                                }
                            });
                    }
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
        if self.demo_enable {
            if let Some(msg) =
                self.demo_source.pull_demo_msg(self.demo_interval_secs)
            {
                new_msgs.push_back(msg);
            }
            while network.pull_ws_message().is_some() {}
        } else {
            while let Some(msg) = network.pull_ws_message() {
                new_msgs.push_back(msg);
            }
        }

        if !self.pause {
            while let Some(msg) = self.message_waiting.pop_front() {
                self.message.push_back((msg, Instant::now(), false));
            }
            while let Some(msg) = new_msgs.pop_front() {
                self.message.push_back((msg, Instant::now(), false));
            }

            while let Some((_, arrive_at, _)) = self.message.front() {
                if arrive_at.elapsed().as_secs_f64()
                    < self.msg_send_delay_secs
                {
                    break;
                }
                let Some((msg, arrive_at, delete)) =
                    self.message.pop_front()
                else {
                    break;
                };

                assert!(
                    arrive_at.elapsed().as_secs_f64()
                        >= self.msg_send_delay_secs
                );
                assert!(!delete);

                network.broadcast_ws_message(msg.clone());
                network.write_log(msg, false);
            }
        } else {
            self.message_waiting.extend(new_msgs);
        }

        if self.demo_settings_show {
            Window::new("Demo Settings")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    if ui
                        .checkbox(&mut self.demo_enable, "Enable")
                        .changed()
                    {
                        ui.data_mut(|d| {
                            d.insert_persisted(
                                self.demo_enable_id,
                                self.demo_enable,
                            )
                        });
                    }

                    ui.label("Send Interval(secs)");
                    let res = ui.add(
                        DragValue::new(&mut self.demo_interval_secs)
                            .min_decimals(1)
                            .max_decimals(2)
                            .range(0.01..=1000.0)
                            .speed(0.01),
                    );
                    if res.changed() {
                        ui.data_mut(|d| {
                            d.insert_persisted(
                                self.demo_interval_secs_id,
                                self.demo_interval_secs,
                            )
                        });
                    }

                    ui.separator();

                    if ui.button("Close").clicked() {
                        self.demo_settings_show = false;
                        ui.data_mut(|d| {
                            d.insert_persisted(
                                self.demo_settings_show_id,
                                self.demo_settings_show,
                            )
                        });
                    }
                });
        }

        CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Send delay(secs): ");
                let drag_value_res = ui.add(
                    DragValue::new(&mut self.msg_send_delay_secs)
                        .min_decimals(1)
                        .max_decimals(1)
                        .range(0.1..=1000.0)
                        .speed(0.1)
                        .update_while_editing(false),
                );
                if drag_value_res.changed() {
                    ui.data_mut(|d| {
                        d.insert_persisted(
                            self.msg_send_delay_secs_id,
                            self.msg_send_delay_secs,
                        )
                    });
                }

                ui.separator();

                if ui.button("Demo Settings").clicked() {
                    self.demo_settings_show = true;
                    ui.data_mut(|d| {
                        d.insert_persisted(
                            self.demo_settings_show_id,
                            self.demo_settings_show,
                        )
                    });
                }
                if self.demo_enable {
                    ui.separator();
                    ui.label(
                        RichText::new("Demo").color(Color32::LIGHT_GREEN),
                    );
                }

                ui.separator();

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
                        / self.msg_send_delay_secs)
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

                self.message.iter().for_each(|(msg, _, delete)| {
                    if *delete {
                        network.write_log(msg.clone(), true);
                    }
                });
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
            pub fn write_log(&self, msg: String, is_delete: bool);
            pub fn restart_server(&self) -> anyhow::Result<()>;
            pub fn restart_ws_client(&self) -> anyhow::Result<()>;
            pub fn stop(self);
        }
    }
}
