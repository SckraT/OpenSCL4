//! The device art in ui/art is the Android app's (android/app/src/main/res/drawable/sound_art_*.xml), converted from
//! its vector drawables to SVG, which Slint draws. This test checks they still match; run it with UPDATE_ART=1 to
//! convert them again after the Android art changes.

use std::{fs, path::Path};

use openscq30_desktop::model::device_art::DeviceArt;

const ANDROID: &str = "http://schemas.android.com/apk/res/android";
const AAPT: &str = "http://schemas.android.com/aapt";

/// #RGB, #RRGGBB or #AARRGGBB as an SVG color and, when it is not opaque, an opacity.
fn color(value: &str) -> (String, Option<String>) {
    let hex = value.trim_start_matches('#');
    let hex: String = if hex.len() == 3 {
        hex.chars().flat_map(|c| [c, c]).collect()
    } else {
        hex.to_string()
    };
    match hex.len() {
        6 => (format!("#{}", hex.to_lowercase()), None),
        8 => {
            let alpha = u8::from_str_radix(&hex[..2], 16).unwrap();
            let opacity = format!("{:.3}", f64::from(alpha) / 255.0);
            let opacity = opacity
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string();
            (
                format!("#{}", hex[2..].to_lowercase()),
                (alpha != 255).then_some(opacity),
            )
        }
        _ => panic!("unknown color {value}"),
    }
}

fn convert(xml: &str, id_prefix: &str) -> String {
    let document = roxmltree::Document::parse(xml).unwrap();
    let vector = document.root_element();
    let attr =
        |node: roxmltree::Node, name: &str| node.attribute((ANDROID, name)).map(str::to_string);
    let (width, height) = (
        attr(vector, "viewportWidth").unwrap(),
        attr(vector, "viewportHeight").unwrap(),
    );
    let mut defs = String::new();
    let mut paths = String::new();
    for (index, path) in vector.children().filter(|n| n.is_element()).enumerate() {
        assert_eq!(
            "path",
            path.tag_name().name(),
            "{id_prefix}: only paths are converted"
        );
        for unsupported in [
            "strokeLineCap",
            "strokeLineJoin",
            "fillAlpha",
            "strokeAlpha",
            "trimPathStart",
        ] {
            assert!(
                attr(path, unsupported).is_none(),
                "{id_prefix}: {unsupported} is not converted"
            );
        }
        let mut parts = vec![format!("d=\"{}\"", attr(path, "pathData").unwrap())];
        let gradient = path
            .children()
            .find(|n| {
                n.tag_name().namespace() == Some(AAPT)
                    && n.attribute("name") == Some("android:fillColor")
            })
            .and_then(|attr_node| {
                attr_node
                    .children()
                    .find(|n| n.tag_name().name() == "gradient")
            });
        if let Some(gradient) = gradient {
            let id = format!("{id_prefix}{index}");
            let stops: String = gradient
                .children()
                .filter(|n| n.tag_name().name() == "item")
                .map(|item| {
                    let (rgb, opacity) = color(&attr(item, "color").unwrap());
                    let opacity = opacity
                        .map(|o| format!(" stop-opacity=\"{o}\""))
                        .unwrap_or_default();
                    format!(
                        "<stop offset=\"{}\" stop-color=\"{rgb}\"{opacity}/>",
                        attr(item, "offset").unwrap()
                    )
                })
                .collect();
            let get = |name| attr(gradient, name).unwrap();
            match attr(gradient, "type").as_deref() {
                Some("radial") => defs.push_str(&format!(
                    "<radialGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" cx=\"{}\" cy=\"{}\" r=\"{}\">{stops}</radialGradient>",
                    get("centerX"),
                    get("centerY"),
                    get("gradientRadius"),
                )),
                Some("linear") | None => defs.push_str(&format!(
                    "<linearGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\">{stops}</linearGradient>",
                    get("startX"),
                    get("startY"),
                    get("endX"),
                    get("endY"),
                )),
                Some(other) => panic!("{id_prefix}: gradient type {other}"),
            }
            parts.push(format!("fill=\"url(#{id})\""));
        } else if let Some(fill) = attr(path, "fillColor") {
            let (rgb, opacity) = color(&fill);
            parts.push(format!("fill=\"{rgb}\""));
            if let Some(opacity) = opacity {
                parts.push(format!("fill-opacity=\"{opacity}\""));
            }
        } else {
            parts.push("fill=\"none\"".to_string());
        }
        if let Some(stroke) = attr(path, "strokeColor") {
            let (rgb, opacity) = color(&stroke);
            parts.push(format!("stroke=\"{rgb}\""));
            parts.push(format!(
                "stroke-width=\"{}\"",
                attr(path, "strokeWidth").unwrap_or_else(|| "1".to_string())
            ));
            if let Some(opacity) = opacity {
                parts.push(format!("stroke-opacity=\"{opacity}\""));
            }
        }
        if attr(path, "fillType").as_deref() == Some("evenOdd") {
            parts.push("fill-rule=\"evenodd\"".to_string());
        }
        paths.push_str(&format!("<path {}/>", parts.join(" ")));
    }
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {width} {height}\" width=\"{width}\" height=\"{height}\"><defs>{defs}</defs>{paths}</svg>\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_device_art_is_the_android_apps() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let drawables = manifest.join("../android/app/src/main/res/drawable");
        let update = std::env::var_os("UPDATE_ART").is_some();
        for art in DeviceArt::ALL {
            for color in ["silver", "black"] {
                let name = format!("sound_art_{}_{color}", art.drawable_name());
                let xml = fs::read_to_string(drawables.join(format!("{name}.xml"))).unwrap();
                let svg = convert(&xml, &format!("{name}-"));
                let target = manifest.join("ui/art").join(format!("{name}.svg"));
                if update {
                    fs::create_dir_all(target.parent().unwrap()).unwrap();
                    fs::write(&target, svg).unwrap();
                } else {
                    let committed = fs::read_to_string(&target)
                        .unwrap_or_default()
                        .replace("\r\n", "\n");
                    assert!(
                        committed == svg,
                        "{name}.svg differs from the Android drawable; run with UPDATE_ART=1"
                    );
                }
            }
        }
    }
}
