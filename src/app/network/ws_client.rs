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

    const MSGS: &[&str] = &[
        "兰茶荼",
        "兰萨卡",
        "兰非拉",
        "兰拉迦",
        "兰拉娜",
        "兰拉吉",
        "兰加惟",
        "兰多摩",
        "兰伊舍",
        "兰纳迦",
        "兰利遮",
        "兰纳真",
        "兰迦鲁",
        "兰般度",
        "兰伽卢",
        "兰贡迪",
        "兰犍多",
        "兰难世",
        "兰梨娄",
        "兰玛尼",
        "兰陀娑",
        "兰耶娑",
        "兰雅玛",
        "兰玛哈",
        "兰帝裟",
        "兰钵答",
        "兰随尼",
        "兰羯磨",
        "兰耶师",
        "兰宁巴",
        "兰沙恭",
        "兰提沙",
        "兰阐荼",
        "兰沙陀",
        "兰沙诃",
        "兰耶多",
        "兰耆都",
        "兰卑浮",
        "兰阿帕斯",
        "兰帕卡提",
        "兰百梨迦",
        "兰多希陀",
        "兰修提袈",
        "兰弥纳离",
        "兰陀尼什",
        "兰穆护昆达",
        "冒失的兰那罗",
        "收集材料的兰那罗",
        "爱音乐的兰那罗",
        "迷茫的兰那罗",
        "淘气的兰那罗",
    ];

    let fut = async move {
        let mut interval = interval(Duration::from_millis(100));
        let mut seed = 114514_u32;

        loop {
            select! {
                _ = interval.tick() => {
                    seed = (seed.wrapping_mul(1103515245) + 12345) % (2_u32.pow(31));
                    let result = message_tx.send(MSGS[seed as usize % MSGS.len()].to_string());
                    if result.is_err() {
                        break;
                    }
                    egui_ctx.request_repaint();
                }
                _ = stop_token_cloned.cancelled() => {
                    break; }
            }
        }

        Ok(())
    };

    (stop_token, fut)
}
