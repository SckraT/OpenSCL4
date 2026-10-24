//! Rendering of the Windows resource script, shared by build.rs and its test.

use std::path::Path;

pub fn render_resource_script(template: &str, version: &str, icon_path: &Path) -> String {
    let (major, minor, patch) = parse_version(version);
    // rc.exe treats backslashes in quoted paths as escapes; forward slashes are accepted on Windows.
    let icon_path = icon_path.to_string_lossy().replace('\\', "/");
    template
        .replace("${ICON}", &icon_path)
        .replace("${VERSION}", version)
        .replace("${VERSION_MAJOR}", major)
        .replace("${VERSION_MINOR}", minor)
        .replace("${VERSION_PATCH}", patch)
}

fn parse_version(version: &str) -> (&str, &str, &str) {
    let mut parts = version.split('.');
    let mut next = || parts.next().expect("the version should have three parts");
    (next(), next(), next())
}
