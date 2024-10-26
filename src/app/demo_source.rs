use std::{env::current_dir, time::Instant};

use anyhow::Context;
use rand::{rngs::StdRng, Rng, SeedableRng};
use tracing::debug;

pub struct DemoSource {
    last_time: Instant,
    rng: StdRng,

    demo_data: Option<Vec<String>>,
}

impl Default for DemoSource {
    fn default() -> Self {
        let get_demo_data = || {
            let data = std::fs::read_to_string(
                current_dir()
                    .context("failed to get cwd")?
                    .join("demo.txt"),
            )
            .context("failed to read demo file")?;

            anyhow::Result::<_>::Ok(
                data.lines()
                    .map(|it| it.to_string())
                    .collect::<Vec<String>>(),
            )
        };

        let demo_data =
            match get_demo_data().context("failed to read demo file") {
                Ok(demo_data) if !demo_data.is_empty() => Some(demo_data),
                Ok(_) => None,
                Err(err) => {
                    debug!("{err:?}");
                    None
                }
            };

        Self {
            last_time: Instant::now(),
            rng: StdRng::from_entropy(),

            demo_data,
        }
    }
}

impl DemoSource {
    pub fn pull_demo_msg(
        &mut self,
        interval_secs: f64,
    ) -> Option<String> {
        if self.last_time.elapsed().as_secs_f64() >= interval_secs {
            self.last_time = Instant::now();
            if let Some(data) = &self.demo_data {
                let idx = self.rng.gen_range(0..data.len());
                Some(data[idx].to_string())
            } else {
                let idx = self.rng.gen_range(0..MSGS.len());
                Some(MSGS[idx].to_string())
            }
        } else {
            None
        }
    }
}

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
    "兰宵宫",
];
