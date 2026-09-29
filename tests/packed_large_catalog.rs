use rust_i18n::t;

rust_i18n::i18n!("benches/fixtures/large_catalog");

#[test]
fn packed_large_catalog_preserves_default_backend_lookups() {
    assert_eq!(t!("orders.save_1", locale = "en"), "Text orders.save_1");
    assert_eq!(t!("orders.save_1", locale = "zh-CN"), "文本 orders.save_1");
    assert_eq!(
        t!("account.title_0", name = "Nick"),
        "Text account.title_0 Nick"
    );
}
