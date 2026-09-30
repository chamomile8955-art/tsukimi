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
                    value.split_whitespace().all(|part| matches!(part, "0" | "999px")),
                    "Independent radius: {}",
                    &declaration[0]
                );
            }
        }
    }

    #[test]
    fn navigation_templates_share_the_same_control_style() {
        let navigation = regex::Regex::new(r#"<class\s+name="navigation-switcher"\s*/>"#).unwrap();
        let segmented = regex::Regex::new(r#"<class\s+name="segmented-control"\s*/>"#).unwrap();
        for template in [
            include_str!("../../../../resources/ui/window.ui"),
            include_str!("../../../../resources/ui/mpv_control_sidebar.ui"),
            include_str!("../../../../resources/ui/list.ui"),
        ] {
            assert!(navigation.is_match(template));
            assert!(segmented.is_match(template));
        }
    }
}
