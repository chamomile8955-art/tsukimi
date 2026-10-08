mod switcher;

pub use switcher::{ThemeSwitcher, apply_theme, normalized_theme};

// Custom poster snapshots must use the same geometry as CSS surfaces.
pub const SURFACE_CORNER_RADIUS: f32 = 14.0;

#[cfg(test)]
mod tests {
    use super::SURFACE_CORNER_RADIUS;

    #[test]
    fn custom_drawing_matches_css_surface_radius() {
        let css = include_str!("../../../../resources/style.css");
        assert!(css.contains(&format!(
            "--tsukimi-surface-radius: {}px;",
            SURFACE_CORNER_RADIUS
        )));
    }

    #[test]
    fn readable_density_is_shared_without_a_second_theme_scale() {
        let css = include_str!("../../../../resources/style.css");
        for declaration in [
            "font-size: 1.25em;",
            "--tsukimi-control-size: 55px;",
            "--tsukimi-tool-size: 68px;",
            "--tsukimi-play-size: 88px;",
            "--tsukimi-window-control-size: 30px;",
        ] {
            assert!(css.contains(declaration), "Missing density: {declaration}");
        }
        for css in [
            include_str!("../../../../resources/style-dark.css"),
            include_str!("../../../../resources/style-settings.css"),
        ] {
            assert!(!css.contains("font-size: 1.25em;"));
            assert!(!css.contains("--tsukimi-control-size:"));
            assert!(!css.contains("--tsukimi-tool-size:"));
            assert!(!css.contains("--tsukimi-play-size:"));
            assert!(!css.contains("--tsukimi-window-control-size:"));
            assert!(!css.contains("--tsukimi-transport-size:"));
            assert!(!css.contains("--tsukimi-theme-swatch-size:"));
        }
        let (width, height) = crate::ui::widgets::utils::TU_ITEM_POST_SIZE;
        assert_eq!(width * 3, height * 2, "Poster aspect ratio");
        let (width, height) = crate::ui::widgets::utils::TU_ITEM_VIDEO_SIZE;
        assert_eq!(width * 9, height * 16, "Backdrop aspect ratio");
    }

    #[test]
    fn styles_do_not_introduce_independent_surface_radii() {
        let declarations = regex::Regex::new(r"border(?:-[a-z]+)*-radius\s*:\s*([^;]+);").unwrap();
        for css in [
            include_str!("../../../../resources/style.css"),
            include_str!("../../../../resources/style-dark.css"),
            include_str!("../../../../resources/style-settings.css"),
        ] {
            for declaration in declarations.captures_iter(css) {
                let value = declaration[1].replace("var(--tsukimi-surface-radius)", "0");
                assert!(
                    value
                        .split_whitespace()
                        .all(|part| matches!(part, "0" | "999px")),
                    "Independent radius: {}",
                    &declaration[0]
                );
            }
        }
    }

    #[test]
    fn navigation_controls_share_the_same_style() {
        let navigation = regex::Regex::new(r#"<class\s+name="navigation-switcher"\s*/>"#).unwrap();
        let segmented = regex::Regex::new(r#"<class\s+name="segmented-control"\s*/>"#).unwrap();
        // Both the main tabs and the player utility tabs live in window.ui.
        let template = include_str!("../../../../resources/ui/window.ui");
        assert_eq!(navigation.find_iter(template).count(), 2);
        assert!(segmented.find_iter(template).count() >= 2);
        let library_classes = crate::ui::widgets::list::LIBRARY_TAB_CLASSES;
        assert!(library_classes.contains(&"navigation-switcher"));
        assert!(library_classes.contains(&"segmented-control"));
    }
}
