# Extend a dependency's translations

This example shows an application overriding translations from a dependency
without changing the dependency's `t!` calls.

The application attaches its backend to the dependency at startup:

```rust
rust_i18n::extend!(my_component);
```

`app` aliases `ui-component` as `my-component`, so its locale namespace uses
the custom Rust crate name:

```yaml
my_component:
  Widget:
    title:
      en: Customized title
      zh-CN: 自定义标题
```

Only translations below `my_component` extend the dependency. Other application
keys remain private to the application backend.

`app-default` depends on `ui-component` without renaming it and uses the default
Rust crate name in both places:

```rust
rust_i18n::extend!(ui_component);
```

```yaml
ui_component:
  Widget:
    title:
      zh-CN: 默认名称自定义标题
```
