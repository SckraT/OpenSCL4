//! The Windows resource script is rendered into OUT_DIR so unchanged builds stay fresh. The icon is
//! referenced by its absolute path because the generated script no longer lives next to it.

#[path = "../build_support.rs"]
mod build_support;

use std::path::Path;

const TEMPLATE: &str = r#"icon ICON "${ICON}"
1 VERSIONINFO
FILEVERSION ${VERSION_MAJOR},${VERSION_MINOR},${VERSION_PATCH},0
VALUE "FileVersion", "${VERSION}"
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_absolute_icon_path_and_version_fields() {
        let rendered = build_support::render_resource_script(
            TEMPLATE,
            "10.2.34",
            Path::new(r"C:\repo\desktop\resources\icon.ico"),
        );

        assert_eq!(
            rendered,
            r#"icon ICON "C:/repo/desktop/resources/icon.ico"
1 VERSIONINFO
FILEVERSION 10,2,34,0
VALUE "FileVersion", "10.2.34"
"#
        );
    }

    #[test]
    fn rendering_the_same_inputs_is_repeatable() {
        let icon = Path::new(r"C:\repo\desktop\resources\icon.ico");

        assert_eq!(
            build_support::render_resource_script(TEMPLATE, "2.13.0", icon),
            build_support::render_resource_script(TEMPLATE, "2.13.0", icon)
        );
    }
}
