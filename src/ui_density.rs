// Pre-compact geometry from 58011eb, including painted padding and borders.
pub const BASELINE_REVISION: &str = "58011eb";
const SCALE_NUMERATOR: i32 = 5;
const SCALE_DENOMINATOR: i32 = 4;
pub const SCALE: f64 = SCALE_NUMERATOR as f64 / SCALE_DENOMINATOR as f64;

pub const fn enlarged(original: i32) -> i32 {
    (original * SCALE_NUMERATOR + SCALE_DENOMINATOR / 2) / SCALE_DENOMINATOR
}

pub const CONTROL_SIZE: i32 = enlarged(44);
pub const TOOL_SIZE: i32 = enlarged(54);
pub const TRANSPORT_SIZE: i32 = enlarged(62);
pub const PLAY_SIZE: i32 = enlarged(70);
pub const PLAY_ICON_SIZE: i32 = enlarged(36);
pub const WINDOW_CONTROL_SIZE: i32 = enlarged(24);
pub const THEME_SWATCH_SIZE: i32 = enlarged(60);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enlargement_uses_original_not_compact_dimensions() {
        assert_eq!(CONTROL_SIZE, 55);
        assert_eq!(TOOL_SIZE, 68);
        assert_eq!(TRANSPORT_SIZE, 78);
        assert_eq!(PLAY_SIZE, 88);
        assert_eq!(PLAY_ICON_SIZE, 45);
        assert_eq!(WINDOW_CONTROL_SIZE, 30);
        assert_eq!(THEME_SWATCH_SIZE, 75);
        assert!(TOOL_SIZE > enlarged(40));
        assert!(PLAY_SIZE > enlarged(48));
    }

    #[test]
    fn styles_match_the_original_enlargement_baseline() {
        let css = include_str!("../resources/style.css");
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
