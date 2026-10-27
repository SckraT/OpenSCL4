//! Every string the window translates with @tr() is translated to every language in translations/, with the same
//! placeholders, the same operating systems named and as many plural forms as the language has, and no translation
//! file has strings the window no longer uses.

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

/// An entry of a .po file whose strings are on one line each. For plurals, the msgid is the singular (the first
/// string of the @tr) and there is a translation for each plural form.
struct Entry {
    id: String,
    plural: bool,
    translations: Vec<String>,
}

fn po_entries(text: &str) -> Vec<Entry> {
    let quoted = |line: &str, key: &str| {
        line.strip_prefix(key).map(|s| {
            s.trim()
                .trim_start_matches('"')
                .trim_end_matches('"')
                .to_string()
        })
    };
    let mut entries: Vec<Entry> = Vec::new();
    for line in text.lines() {
        if let Some(id) = quoted(line, "msgid ") {
            entries.push(Entry {
                id,
                plural: false,
                translations: Vec::new(),
            });
        } else if line.starts_with("msgid_plural ") {
            if let Some(entry) = entries.last_mut() {
                entry.plural = true;
            }
        } else if let Some(value) = quoted(line, "msgstr ").or_else(|| {
            let rest = line.strip_prefix("msgstr[")?;
            quoted(rest.split_once("] ")?.1, "")
        }) && let Some(entry) = entries.last_mut()
        {
            entry.translations.push(value);
        }
    }
    entries.retain(|e| !e.id.is_empty());
    entries
}

/// How many plural forms the .po file's header says its language has.
fn plural_forms(text: &str) -> usize {
    let header = text
        .split("nplurals=")
        .nth(1)
        .expect("the header names the plural forms");
    header[..1].parse().unwrap()
}

/// The operating systems a string may name. A translation names one exactly when its string does, so wording reworded
/// for another system cannot keep the old name in a translation.
const SYSTEMS: [&str; 2] = ["Windows", "Linux"];

/// The placeholders of a string in order, such as ["{}", "{}"], which a translation must keep.
fn placeholders(text: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        let Some(end) = rest[start..].find('}') else {
            break;
        };
        found.push(&rest[start..start + end + 1]);
        rest = &rest[start + end + 1..];
    }
    found.sort_unstable();
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_interface_string_is_translated_to_every_language() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut strings = BTreeSet::new();
        ui_strings(&manifest.join("ui"), &mut strings);
        let mut languages = 0;
        for language in fs::read_dir(manifest.join("translations")).unwrap() {
            let language = language.unwrap();
            let name = language.file_name().to_string_lossy().to_string();
            let po = fs::read_to_string(language.path().join("LC_MESSAGES/openscq30-desktop.po"))
                .unwrap();
            languages += 1;
            let entries = po_entries(&po);
            let forms = plural_forms(&po);

            let translated: BTreeSet<_> = entries
                .iter()
                .filter(|e| {
                    !e.translations.is_empty() && e.translations.iter().all(|t| !t.is_empty())
                })
                .map(|e| e.id.clone())
                .collect();
            let missing: Vec<_> = strings.difference(&translated).collect();
            assert!(
                missing.is_empty(),
                "{name} misses translations: {missing:?}"
            );

            let unused: Vec<_> = entries
                .iter()
                .map(|e| &e.id)
                .filter(|id| !strings.contains(*id))
                .collect();
            assert!(
                unused.is_empty(),
                "{name} has translations the window no longer uses: {unused:?}"
            );

            for entry in &entries {
                for translation in &entry.translations {
                    for system in SYSTEMS {
                        assert_eq!(
                            entry.id.contains(system),
                            translation.contains(system),
                            "{name} names {system} differently from {:?}: {translation:?}",
                            entry.id
                        );
                    }
                }
                if entry.plural {
                    assert_eq!(
                        forms,
                        entry.translations.len(),
                        "{name} has the wrong number of plural forms for {:?}",
                        entry.id
                    );
                    if forms > 1 {
                        for translation in &entry.translations {
                            assert!(
                                translation.contains("{n}"),
                                "{name}: a plural form of {:?} lacks the number: {translation:?}",
                                entry.id
                            );
                        }
                    }
                } else {
                    for translation in &entry.translations {
                        assert_eq!(
                            placeholders(&entry.id),
                            placeholders(translation),
                            "{name} changes the placeholders of {:?}: {translation:?}",
                            entry.id
                        );
                        assert_eq!(
                            entry.id.contains("\\n"),
                            translation.contains("\\n"),
                            "{name} changes the line breaks of {:?}",
                            entry.id
                        );
                    }
                }
            }
        }
        assert!(languages >= 15, "only {languages} languages");
    }
}
