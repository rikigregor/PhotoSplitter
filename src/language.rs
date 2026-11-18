use std::sync::OnceLock;

pub fn for_locale(locale: &str) -> &'static str {
    if locale
        .split(['-', '_', '.', '@'])
        .next()
        .unwrap_or("")
        .eq_ignore_ascii_case("sk")
    {
        "sk"
    } else {
        "en"
    }
}

pub fn code() -> &'static str {
    static LANGUAGE: OnceLock<&'static str> = OnceLock::new();
    LANGUAGE.get_or_init(|| {
        if let Ok(value) = std::env::var("PHOTOSPLITTER_LANG") {
            return for_locale(&value);
        }
        #[cfg(windows)]
        {
            #[link(name = "kernel32")]
            extern "system" {
                fn GetUserDefaultLocaleName(buffer: *mut u16, length: i32) -> i32;
            }
            let mut buffer = [0u16; 85];
            let len = unsafe { GetUserDefaultLocaleName(buffer.as_mut_ptr(), buffer.len() as i32) };
            if len > 1 {
                return for_locale(&String::from_utf16_lossy(&buffer[..len as usize - 1]));
            }
        }
        "en"
    })
}
pub fn text(en: &'static str, sk: &'static str) -> &'static str {
    if code() == "sk" {
        sk
    } else {
        en
    }
}
pub fn catalog() -> serde_json::Value {
    serde_json::from_str(include_str!("localization.json")).expect("valid localization resources")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locale_selection() {
        for l in ["sk-SK", "sk", "SK_sk", "sk_SK.UTF-8"] {
            assert_eq!(for_locale(l), "sk");
        }
        for l in ["en-US", "cs-CZ", "de-DE", "", "unknown"] {
            assert_eq!(for_locale(l), "en");
        }
    }
    #[test]
    fn both_ui_catalogs_have_the_same_keys() {
        let c = catalog();
        let en = c["en"]["ui"].as_object().unwrap();
        let sk = c["sk"]["ui"].as_object().unwrap();
        assert_eq!(en.keys().collect::<Vec<_>>(), sk.keys().collect::<Vec<_>>());
        for (key, value) in en {
            assert!(!value.as_str().unwrap().is_empty(), "{key}");
            assert!(!sk[key].as_str().unwrap().is_empty(), "{key}");
        }
    }
}
