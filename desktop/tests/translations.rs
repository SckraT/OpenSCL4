//! Every string the window translates with @tr() has a Russian translation in translations/ru, and the translation
//! file has nothing the window no longer uses.

use std::{collections::BTreeSet, fs, path::Path};

/// The first argument of each @tr("…") in the .slint files, as written there.
fn ui_strings(dir: &Path, found: &mut BTreeSet<String>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            ui_strings(&path, found);
        } else if path.extension().is_some_and(|e| e == "slint") {
            let text = fs::read_to_string(&path).unwrap();
            let mut rest = text.as_str();
            while let Some(start) = rest.find("@tr(") {
                rest = rest[start + 4..].trim_start();
                let string = rest.strip_prefix('"').expect("@tr takes a string");
                let mut end = 0;
                let bytes = string.as_bytes();
                while bytes[end] != b'"' {
                    end += if bytes[end] == b'\\' { 2 } else { 1 };
                }
                found.insert(string[..end].to_string());
                rest = &string[end..];
            }
        }
    }
}

/// msgid and msgstr pairs of a .po file whose entries are on one line each. For plurals, the msgid is the singular
/// (the first string of the @tr) and the msgstr is the first form.
fn po_entries(text: &str) -> Vec<(String, String)> {
    let quoted = |line: &str, key: &str| {
        line.strip_prefix(key).map(|s| {
            s.trim()
                .trim_start_matches('"')
                .trim_end_matches('"')
                .to_string()
        })
    };
    let mut entries = Vec::new();
    let mut id = None;
    for line in text.lines() {
        if let Some(value) = quoted(line, "msgid ") {
            id = Some(value);
        } else if let Some(value) = quoted(line, "msgstr ").or_else(|| quoted(line, "msgstr[0] "))
            && let Some(msgid) = id.take()
            && !msgid.is_empty()
        {
            entries.push((msgid, value));
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_interface_string_is_translated_to_russian() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut strings = BTreeSet::new();
        ui_strings(&manifest.join("ui"), &mut strings);
        let po =
            fs::read_to_string(manifest.join("translations/ru/LC_MESSAGES/openscq30-desktop.po"))
                .unwrap();
        let entries = po_entries(&po);
        let translated: BTreeSet<_> = entries
            .iter()
            .filter(|(_, s)| !s.is_empty())
            .map(|(id, _)| id.clone())
            .collect();
        let missing: Vec<_> = strings.difference(&translated).collect();
        assert!(
            missing.is_empty(),
            "missing Russian translations:\n{}",
            missing
                .iter()
                .map(|s| format!("msgid \"{s}\"\nmsgstr \"\"\n"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        let unused: Vec<_> = entries
            .iter()
            .map(|(id, _)| id)
            .filter(|id| !strings.contains(*id))
            .collect();
        assert!(
            unused.is_empty(),
            "translations the window no longer uses: {unused:?}"
        );
    }
}
