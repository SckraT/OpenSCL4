//! The languages the app speaks: the ones the library and gui/ are translated to, each named in itself.

/// A language by the name of its folder in translations/ and lib/i18n/, and its own name for itself.
pub struct Language {
    pub code: &'static str,
    pub name: &'static str,
    /// Whether it writes 1,5 rather than 1.5.
    pub decimal_comma: bool,
}

const fn language(code: &'static str, name: &'static str, decimal_comma: bool) -> Language {
    Language {
        code,
        name,
        decimal_comma,
    }
}

pub const LANGUAGES: [Language; 16] = [
    language("en", "English", false),
    language("arz", "مصري", false),
    language("bn", "বাংলা", false),
    language("de", "Deutsch", true),
    language("es", "Español", true),
    language("fr", "Français", true),
    language("he", "עברית", false),
    language("id", "Bahasa Indonesia", true),
    language("it", "Italiano", true),
    language("ja", "日本語", false),
    language("ko", "한국어", false),
    language("pl", "Polski", true),
    language("pt-BR", "Português (Brasil)", true),
    language("ru", "Русский", true),
    language("tr", "Türkçe", true),
    language("uk", "Українська", true),
];

pub fn find(code: &str) -> Option<&'static Language> {
    LANGUAGES.iter().find(|l| l.code.eq_ignore_ascii_case(code))
}

/// The app's language for the languages the system asks for, such as "pt-BR" or "de-AT", in order of preference:
/// the first one the app has, by its whole tag or else by its language. English when it has none of them.
pub fn choose(requested: &[String]) -> &'static Language {
    requested
        .iter()
        .find_map(|tag| {
            find(tag).or_else(|| {
                let primary = tag.split(['-', '_']).next().unwrap_or_default();
                // the old codes of Hebrew and Indonesian, Egyptian Arabic for Arabic, and Portuguese as in Brazil
                let primary = match primary.to_ascii_lowercase().as_str() {
                    "iw" => "he",
                    "in" => "id",
                    "ar" => "arz",
                    "pt" => "pt-BR",
                    _ => primary,
                };
                find(primary)
            })
        })
        .unwrap_or(&LANGUAGES[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chosen(tags: &[&str]) -> &'static str {
        choose(&tags.iter().map(ToString::to_string).collect::<Vec<_>>()).code
    }

    #[test]
    fn takes_the_first_language_the_app_has() {
        assert_eq!("de", chosen(&["de-AT", "ru"]));
        assert_eq!("ru", chosen(&["kk", "ru-RU"]));
        assert_eq!("pt-BR", chosen(&["pt-BR"]));
        assert_eq!("pt-BR", chosen(&["pt-PT"]));
        assert_eq!("he", chosen(&["iw"]));
        assert_eq!("arz", chosen(&["ar-EG"]));
        assert_eq!("en", chosen(&["kk"]));
        assert_eq!("en", chosen(&[]));
    }

    #[test]
    fn every_language_has_its_translations() {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        for language in LANGUAGES.iter().skip(1) {
            let po = manifest.join(format!(
                "translations/{}/LC_MESSAGES/openscq30-desktop.po",
                language.code
            ));
            assert!(po.exists(), "{} has no translations", language.code);
            assert!(
                manifest
                    .join(format!("../lib/i18n/{}", language.code))
                    .exists(),
                "the library has no {}",
                language.code
            );
        }
    }
}
