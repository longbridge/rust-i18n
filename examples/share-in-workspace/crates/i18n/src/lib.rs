use rust_i18n::Backend;
use std::borrow::Cow;

rust_i18n::i18n!("../../locales");

pub struct I18nBackend;

impl Backend for I18nBackend {
    fn available_locales(&self) -> Vec<Cow<'_, str>> {
        _RUST_I18N_BACKEND.available_locales()
    }

    fn translate(&self, locale: &str, key: &str) -> Option<Cow<'_, str>> {
        let val = _RUST_I18N_BACKEND.translate(locale, key);
        if val.is_none() {
            _RUST_I18N_BACKEND.translate("en", key)
        } else {
            val
        }
    }
}

#[macro_export]
macro_rules! init {
    () => {
        rust_i18n::i18n!(backend = i18n::I18nBackend);
    };
}

pub use rust_i18n::set_locale;
pub use rust_i18n::t;
