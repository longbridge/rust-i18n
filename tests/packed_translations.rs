use rust_i18n::t;

fn unpack_translations() -> rust_i18n::SimpleBackend {
    rust_i18n::SimpleBackend::new()
}

// A custom backend keeps this small fixture on the packed backend path.
rust_i18n::i18n!("tests/packed_locales", backend = unpack_translations());

#[test]
fn packed_translations_preserve_special_characters() {
    assert_eq!(t!("delimiter;key,1", locale = "en"), "12;34,56");
    assert_eq!(t!("unicode", locale = "en"), "你好 %{name} 🎉");
    assert_eq!(t!("é;键", locale = "en"), "bien");
    assert_eq!(t!("newline", locale = "en"), "line one\nline two");
    assert_eq!(t!("control", locale = "en"), "\u{0}\u{1}\u{2}");
    assert_eq!(t!("empty", locale = "en"), "");
}
