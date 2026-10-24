//! The Windows resource script is rendered into OUT_DIR so unchanged builds stay fresh. The icon is
//! referenced by its absolute path because the generated script no longer lives next to it. Tests
//! render the shipped script so template drift is caught.

#[path = "../build_support.rs"]
mod build_support;

use std::path::Path;

const TEMPLATE_PATH: &str = "resources/resources.rc";

fn template() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(TEMPLATE_PATH)).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_absolute_icon_path_and_version_fields() {
        let template = template();
        assert!(template.contains(r#"icon ICON "${ICON}""#));
        assert!(
            template.contains("FILEVERSION ${VERSION_MAJOR},${VERSION_MINOR},${VERSION_PATCH},0")
        );
        assert!(template.contains(r#"VALUE "FileVersion", "${VERSION}""#));

        let rendered = build_support::render_resource_script(
            &template,
            "10.2.34",
            Path::new(r"C:\repo\desktop\resources\icon.ico"),
        );

        assert!(rendered.contains(r#"icon ICON "C:/repo/desktop/resources/icon.ico""#));
        assert!(rendered.contains("FILEVERSION 10,2,34,0"));
        assert!(rendered.contains(r#"VALUE "FileVersion", "10.2.34""#));
        assert!(!rendered.contains("${"));
    }

    #[test]
    fn rendering_the_same_inputs_is_repeatable() {
        let icon = Path::new(r"C:\repo\desktop\resources\icon.ico");
        let template = template();

        assert_eq!(
            build_support::render_resource_script(&template, "2.13.0", icon),
            build_support::render_resource_script(&template, "2.13.0", icon)
        );
    }
}
