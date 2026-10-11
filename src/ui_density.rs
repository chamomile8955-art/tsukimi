// Painted logical sizes include padding and borders. GTK applies monitor DPI.
pub const FONT_SCALE: f64 = 1.5;
pub const CONTROL_SIZE: i32 = 50;
pub const TOOL_SIZE: i32 = 60;
pub const TRANSPORT_SIZE: i32 = 70;
pub const PLAY_SIZE: i32 = 80;
pub const PLAY_ICON_SIZE: i32 = 40;
pub const WINDOW_CONTROL_SIZE: i32 = 28;
pub const THEME_SWATCH_SIZE: i32 = 68;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_retain_a_clear_size_hierarchy() {
        assert!(CONTROL_SIZE < TOOL_SIZE);
        assert!(TOOL_SIZE < TRANSPORT_SIZE);
        assert!(TRANSPORT_SIZE < PLAY_SIZE);
        assert_eq!(PLAY_ICON_SIZE * 2, PLAY_SIZE);
    }

    #[test]
    fn styles_match_the_shared_density() {
        let css = include_str!("../resources/style.css");
        assert!(css.contains(&format!("font-size: {FONT_SCALE}em;")));
        for (name, size) in [
            ("control", CONTROL_SIZE),
            ("tool", TOOL_SIZE),
            ("transport", TRANSPORT_SIZE),
            ("play", PLAY_SIZE),
            ("window-control", WINDOW_CONTROL_SIZE),
            ("theme-swatch", THEME_SWATCH_SIZE),
        ] {
            let declaration = format!("--tsukimi-{name}-size: {size}px;");
            assert!(css.contains(&declaration), "Wrong baseline: {declaration}");
        }
        let template = include_str!("../resources/ui/mpvpage.ui");
        assert!(template.contains(&format!(
            "<property name=\"pixel-size\">{PLAY_ICON_SIZE}</property>"
        )));
    }
}
