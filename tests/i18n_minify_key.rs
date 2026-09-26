rust_i18n::i18n!(
    "./tests/locales",
    fallback = "en",
    minify_key = true,
    minify_key_len = 24,
    minify_key_prefix = "t_",
    minify_key_thresh = 4
);

#[cfg(test)]
mod tests {
    use rust_i18n::{t, tkv};

    #[test]
    fn test_i18n_attrs() {
        assert!(crate::_RUST_I18N_MINIFY_KEY);
        assert_eq!(crate::_RUST_I18N_MINIFY_KEY_LEN, 24);
        assert_eq!(crate::_RUST_I18N_MINIFY_KEY_PREFIX, "t_");
        assert_eq!(crate::_RUST_I18N_MINIFY_KEY_THRESH, 4);
    }

    #[test]
    fn test_t() {
        assert_eq!(t!("Bar - Hello, World!"), "Bar - Hello, World!");
        assert_eq!(
            t!("Bar - Hello, World!", locale = "en"),
            "Bar - Hello, World!"
        );
        assert_eq!(
            t!("Bar - Hello, World!", locale = "zh-CN"),
            "Bar - 你好世界！"
        );
        let fruits = ["Apple", "Banana", "Orange"];
        let fruits_translated = ["苹果", "香蕉", "橘子"];
        for (src, dst) in fruits.iter().zip(fruits_translated.iter()) {
            assert_eq!(t!(*src, locale = "zh-CN"), *dst);
        }
        let msg = "aka".to_string();
        let i = 0;
        assert_eq!(t!(msg, name => & i : {} ), "aka");
        assert_eq!(t!("hello"), "hello");
        assert_eq!(t!("hello",), "hello");
        assert_eq!(t!("hello", locale = "en"), "hello");
        assert_eq!(t!(String::from("hello"), locale = "en"), "hello");
        assert_eq!(t!("Hello, %{name}", name = "Bar"), "Hello, Bar");
        assert_eq!(
            t!("You have %{count} messages.", locale = "zh-CN", count = 1 + 2,,,),
            "你收到了 3 条新消息。"
        );
    }

    #[test]
    fn test_tkv() {
        let (key, msg) = tkv!("");
        assert_eq!(key, "");
        assert_eq!(msg, "");
        let (key, msg) = tkv!("Hey");
        assert_eq!(key, "Hey");
        assert_eq!(msg, "Hey");
        let (key, msg) = tkv!("Hello, world!");
        assert_eq!(key, "t_1LokVzuiIrh1xByyZG4wjZ");
        assert_eq!(msg, "Hello, world!");
    }
}
