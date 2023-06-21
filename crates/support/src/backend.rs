use std::collections::HashMap;

/// I18n backend trait
pub trait Backend: Send + Sync + 'static {
    /// Return the available locales
    fn available_locales(&self) -> Vec<&str>;
    /// Get the translation for the given locale and key
    fn translate(&self, locale: &str, key: &str) -> Option<&str>;
}

pub trait BackendExt: Backend {
    /// Extend backend to add more translations
    fn extend<T: Backend>(self, other: T) -> CombinedBackend<Self, T>
    where
        Self: Sized,
    {
        CombinedBackend(self, other)
    }
}

pub struct CombinedBackend<A, B>(A, B);

impl<A, B> Backend for CombinedBackend<A, B>
where
    A: Backend,
    B: Backend,
{
    fn available_locales(&self) -> Vec<&str> {
        let mut available_locales = self.0.available_locales();
        for locale in self.1.available_locales() {
            if !available_locales.contains(&locale) {
                available_locales.push(locale);
            }
        }
        available_locales
    }

    #[inline]
    fn translate(&self, locale: &str, key: &str) -> Option<&str> {
        self.1
            .translate(locale, key)
            .or_else(|| self.0.translate(locale, key))
    }
}

/// Simple KeyValue storage backend
pub struct SimpleBackend {
    /// All translations key is flatten key, like `en.hello.world`
    translations: HashMap<String, HashMap<String, String>>,
}

impl SimpleBackend {
    /// Create a new SimpleBackend.
    pub fn new() -> Self {
        SimpleBackend {
            translations: HashMap::new(),
        }
    }

    /// Add more translations for the given locale.
    ///
    /// ```ignore
    /// let trs = HashMap::<String, String>::new();
    /// trs.insert("hello".into(), "Hello".into());
    /// trs.insert("foo".into(), "Foo bar".into());
    /// backend.add_translations("en", &data);
    /// ```
    pub fn add_translations(&mut self, locale: &str, data: &HashMap<&str, &str>) {
        let data = data
            .clone()
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect::<HashMap<_, _>>();

        if let Some(trs) = self.translations.get_mut(locale) {
            trs.extend(data.clone());
        } else {
            self.translations.insert(locale.into(), data.clone());
        }
    }
}

impl Backend for SimpleBackend {
    fn available_locales(&self) -> Vec<&str> {
        let mut locales = self
            .translations
            .keys()
            .map(|k| k.as_str())
            .collect::<Vec<_>>();
        locales.sort();
        locales
    }

    fn translate(&self, locale: &str, key: &str) -> Option<&str> {
        if let Some(trs) = self.translations.get(locale) {
            return trs.get(key).map(|s| s.as_str());
        }

        None
    }
}

impl BackendExt for SimpleBackend {}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::SimpleBackend;
    use super::{Backend, BackendExt};

    #[test]
    fn test_simple_backend() {
        let mut backend = SimpleBackend::new();
        let mut data = HashMap::<&str, &str>::new();
        data.insert("hello", "Hello");
        data.insert("foo", "Foo bar");
        backend.add_translations("en", &data);

        let mut data_cn = HashMap::<&str, &str>::new();
        data_cn.insert("hello", "你好");
        data_cn.insert("foo", "Foo 测试");
        backend.add_translations("zh-CN", &data_cn);

        assert_eq!(backend.translate("en", "hello"), Some("Hello".into()));
        assert_eq!(backend.translate("en", "foo"), Some("Foo bar".into()));
        assert_eq!(backend.translate("zh-CN", "hello"), Some("你好".into()));
        assert_eq!(backend.translate("zh-CN", "foo"), Some("Foo 测试".into()));

        assert_eq!(backend.available_locales(), vec!["en", "zh-CN"]);
    }

    #[test]
    fn test_combined_backend() {
        let mut backend = SimpleBackend::new();
        let mut data = HashMap::<&str, &str>::new();
        data.insert("hello", "Hello");
        data.insert("foo", "Foo bar");
        backend.add_translations("en", &data);

        let mut data_cn = HashMap::<&str, &str>::new();
        data_cn.insert("hello", "你好");
        data_cn.insert("foo", "Foo 测试");
        backend.add_translations("zh-CN", &data_cn);

        let mut backend2 = SimpleBackend::new();
        let mut data2 = HashMap::<&str, &str>::new();
        data2.insert("hello", "Hello2");
        backend2.add_translations("en", &data2);

        let mut data_cn2 = HashMap::<&str, &str>::new();
        data_cn2.insert("hello", "你好2");
        backend2.add_translations("zh-CN", &data_cn2);

        let combined = backend.extend(backend2);
        assert_eq!(combined.translate("en", "hello"), Some("Hello2".into()));
        assert_eq!(combined.translate("zh-CN", "hello"), Some("你好2".into()));

        assert_eq!(combined.available_locales(), vec!["en", "zh-CN"]);
    }
}
