rust_i18n::i18n!("locales", fallback = "en");

fn main() {
    rust_i18n::extend!(ui_component);
    rust_i18n::set_locale("zh-CN");

    println!("{}", ui_component::title());
    println!("{}", ui_component::description());
}

#[cfg(test)]
mod tests {
    #[test]
    fn extends_a_dependency_from_its_default_crate_namespace() {
        rust_i18n::extend!(ui_component);

        rust_i18n::set_locale("zh-CN");
        assert_eq!(ui_component::title(), "默认名称自定义标题");
        assert_eq!(ui_component::description(), "Component description");
    }
}
