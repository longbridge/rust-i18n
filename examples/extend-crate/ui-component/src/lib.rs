use rust_i18n::t;

rust_i18n::i18n!("locales", fallback = "en");

pub fn title() -> String {
    t!("Widget.title").into_owned()
}

pub fn description() -> String {
    t!("Widget.description").into_owned()
}
