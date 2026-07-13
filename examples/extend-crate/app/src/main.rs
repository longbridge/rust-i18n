rust_i18n::i18n!("locales", fallback = "en");

fn main() {
    rust_i18n::extend!(my_component);
    rust_i18n::set_locale("zh-CN");

    println!("{}", my_component::title());
    println!("{}", my_component::description());
}

#[cfg(test)]
mod tests {
    #[test]
    fn extends_a_dependency_from_its_crate_namespace() {
        rust_i18n::extend!(my_component);

        rust_i18n::set_locale("zh-CN");
        assert_eq!(my_component::title(), "自定义标题");
        assert_eq!(my_component::description(), "Component description");

        rust_i18n::set_locale("en");
        assert_eq!(my_component::title(), "Component title");
    }
}
