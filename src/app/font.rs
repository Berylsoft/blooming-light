use eframe::egui::{Context, FontData, FontDefinitions, FontFamily};

pub fn setup_fonts(ctx: &Context) {
    const SOURCE_HAN_SANS: &[u8] =
        include_bytes!("../../font/SourceHanSans-VF.otf.ttc");
    const JETBRAINS_MONO: &[u8] =
        include_bytes!("../../font/JetBrainsMono[wght].ttf");
    const JETBRAINS_MONO_ITALIC: &[u8] =
        include_bytes!("../../font/JetBrainsMono-Italic[wght].ttf");

    let mut fonts = FontDefinitions::empty();

    fonts.font_data.insert(
        "SourceHanSans-VF".into(),
        FontData::from_static(SOURCE_HAN_SANS),
    );
    fonts.font_data.insert(
        "JetBrainsMono".into(),
        FontData::from_static(JETBRAINS_MONO),
    );
    fonts.font_data.insert(
        "JetBrainsMono-Italic".into(),
        FontData::from_static(JETBRAINS_MONO_ITALIC),
    );

    fonts.families.insert(
        FontFamily::Proportional,
        vec!["SourceHanSans-VF".to_owned()],
    );

    fonts.families.insert(
        FontFamily::Monospace,
        vec![
            "JetBrainsMono".to_owned(),
            "JetBrainsMono-Italic".to_owned(),
        ],
    );

    ctx.set_fonts(fonts);
}
