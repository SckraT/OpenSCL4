fn main() {
    // translations are plain msgids, without the component as their context
    let config = slint_build::CompilerConfiguration::new()
        .with_style("material".into())
        .with_bundled_translations(concat!(env!("CARGO_MANIFEST_DIR"), "/translations"))
        .with_default_translation_context(slint_build::DefaultTranslationContext::None);
    slint_build::compile_with_config("ui/app.slint", config).expect("the UI should compile");
    println!("cargo:rerun-if-changed=translations");

    // Having #[cfg(windows)] means this will not run when cross compiling, but without it, it will have to
    // compile windows dependencies when the target os is linux.
    #[cfg(windows)]
    if std::env::var_os("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        windows();
    }
}

/// The icon and version information of the exe, as gui/build.rs does it.
#[cfg(windows)]
fn windows() {
    use std::fs;
    use std::io::Write;
    use std::path::Path;

    let version = env!("CARGO_PKG_VERSION");
    let (major, minor, patch) = parse_version(version);

    let resource_dir = Path::new("resources");
    let resources = fs::read_to_string(resource_dir.join("resources.rc")).unwrap();
    let resources_with_version = resources
        .replace("${VERSION}", version)
        .replace("${VERSION_MAJOR}", major)
        .replace("${VERSION_MINOR}", minor)
        .replace("${VERSION_PATCH}", patch);

    // The file with text replacements needs to be in the same directory so relative path references work
    let mut temp_resource_file = tempfile::Builder::new().tempfile_in(resource_dir).unwrap();
    write!(temp_resource_file, "{resources_with_version}").unwrap();
    temp_resource_file.flush().unwrap();

    embed_resource::compile(temp_resource_file.path(), embed_resource::NONE)
        .manifest_required()
        .unwrap();
    println!("cargo:rerun-if-changed=resources");
}

#[cfg(windows)]
fn parse_version(version: &str) -> (&str, &str, &str) {
    let mut parts = version.split('.');
    let mut next = || parts.next().expect("the version should have three parts");
    (next(), next(), next())
}
