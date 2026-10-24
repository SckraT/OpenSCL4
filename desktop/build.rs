#[cfg(windows)]
#[path = "build_support.rs"]
mod build_support;

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
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};

    let version = env!("CARGO_PKG_VERSION");

    let resource_dir = Path::new("resources");
    let template = fs::read_to_string(resource_dir.join("resources.rc")).unwrap();
    let icon_path = std::path::absolute(resource_dir.join("com.oppzippy.OpenSCQ30.ico")).unwrap();
    let resources_with_version =
        build_support::render_resource_script(&template, version, &icon_path);

    // OUT_DIR keeps a fixed path, so the generated resource and its link argument are stable
    // across builds. The icon is referenced by absolute path because the script no longer lives
    // next to it.
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let resource_script = out_dir.join("resources.rc");
    fs::write(&resource_script, resources_with_version).unwrap();

    embed_resource::compile(&resource_script, embed_resource::NONE)
        .manifest_required()
        .unwrap();
    println!("cargo:rerun-if-changed=resources/resources.rc");
    println!("cargo:rerun-if-changed=resources/com.oppzippy.OpenSCQ30.ico");
}
