use std::borrow::Cow;
use std::collections::HashMap;

/// A view of another backend restricted to a single namespace.
pub struct NamespacedBackend {
    backend: &'static dyn Backend,
    namespace: &'static str,
}

impl NamespacedBackend {
    /// Create a backend that exposes only keys below `namespace`.
    pub fn new(backend: &'static dyn Backend, namespace: &'static str) -> Self {
        Self { backend, namespace }
    }

    fn namespaced_key(&self, key: &str) -> String {
        format!("{}.{key}", self.namespace)
    }
}

impl Backend for NamespacedBackend {
    fn available_locales(&self) -> Vec<Cow<'_, str>> {
        self.backend
            .available_locales()
            .into_iter()
            .filter(|locale| self.messages_for_locale(locale).is_some())
            .collect()
    }

    fn translate(&self, locale: &str, key: &str) -> Option<Cow<'_, str>> {
        self.backend.translate(locale, &self.namespaced_key(key))
    }

    fn messages_for_locale(&self, locale: &str) -> Option<Vec<(Cow<'_, str>, Cow<'_, str>)>> {
        let prefix = format!("{}.", self.namespace);
        let messages = self
            .backend
            .messages_for_locale(locale)?
            .into_iter()
            .filter_map(|(key, value)| {
                key.strip_prefix(&prefix)
                    .map(|key| (Cow::Owned(key.to_string()), value))
            })
            .collect::<Vec<_>>();

        (!messages.is_empty()).then_some(messages)
    }
}

/// I18n backend trait
pub trait Backend: Send + Sync + 'static {
    /// Return the available locales
    fn available_locales(&self) -> Vec<Cow<'_, str>>;
    /// Get the translation for the given locale and key
    fn translate(&self, locale: &str, key: &str) -> Option<Cow<'_, str>>;
    /// Get all translations for the given locale
    fn messages_for_locale(&self, locale: &str) -> Option<Vec<(Cow<'_, str>, Cow<'_, str>)>>;
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
    fn available_locales(&self) -> Vec<Cow<'_, str>> {
        let mut available_locales = self.0.available_locales();
        for locale in self.1.available_locales() {
            if !available_locales.contains(&locale) {
                available_locales.push(locale);
            }
        }
        available_locales
    }

    #[inline]
    fn translate(&self, locale: &str, key: &str) -> Option<Cow<'_, str>> {
        self.1
            .translate(locale, key)
            .or_else(|| self.0.translate(locale, key))
    }

    fn messages_for_locale(&self, locale: &str) -> Option<Vec<(Cow<'_, str>, Cow<'_, str>)>> {
        match (
            self.1.messages_for_locale(locale),
            self.0.messages_for_locale(locale),
        ) {
            (None, None) => None,
            (None, a) => a,
            (b, None) => b,
            (Some(b), Some(a)) => Some(
                b.into_iter()
                    .chain(
                        a.into_iter()
                            .filter(|(k, _)| self.1.translate(locale, k).is_none()),
                    )
                    .collect(),
            ),
        }
    }
}

/// Simple KeyValue storage backend
pub struct SimpleBackend {
    /// All translations key is flatten key, like `en.hello.world`
    translations: HashMap<Cow<'static, str>, HashMap<Cow<'static, str>, Cow<'static, str>>>,
}

impl
    FromIterator<(
        Cow<'static, str>,
        HashMap<Cow<'static, str>, Cow<'static, str>>,
    )> for SimpleBackend
{
    fn from_iter<
        I: IntoIterator<
            Item = (
                Cow<'static, str>,
                HashMap<Cow<'static, str>, Cow<'static, str>>,
            ),
        >,
    >(
        iter: I,
    ) -> Self {
        Self {
            translations: iter.into_iter().collect(),
        }
    }
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
    /// ```no_run
    /// # use std::collections::HashMap;
    /// # use rust_i18n_support::SimpleBackend;
    /// # let mut backend = SimpleBackend::new();
    /// let mut trs = HashMap::new();
    /// trs.insert("hello".into(), "Hello".into());
    /// trs.insert("foo".into(), "Foo bar".into());
    /// backend.add_translations("en".into(), trs);
    /// ```
    pub fn add_translations(
        &mut self,
        locale: Cow<'static, str>,
        data: HashMap<Cow<'static, str>, Cow<'static, str>>,
    ) {
        let trs = self.translations.entry(locale.into()).or_default();
        trs.extend(data);
    }
}

impl Backend for SimpleBackend {
    fn available_locales(&self) -> Vec<Cow<'_, str>> {
        let mut locales = self.translations.keys().cloned().collect::<Vec<_>>();
        locales.sort();
        locales
    }

    fn translate(&self, locale: &str, key: &str) -> Option<Cow<'_, str>> {
        if let Some(trs) = self.translations.get(locale) {
            return trs.get(key).cloned();
        }

        None
    }

    fn messages_for_locale(&self, locale: &str) -> Option<Vec<(Cow<'_, str>, Cow<'_, str>)>> {
        self.translations
            .get(locale)
            .map(|trs| trs.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
    }
}

impl BackendExt for SimpleBackend {}

impl Default for SimpleBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::collections::HashMap;

    use super::SimpleBackend;
    use super::{Backend, BackendExt, NamespacedBackend};

    #[test]
    fn test_simple_backend() {
        let mut backend = SimpleBackend::new();
        let mut data = HashMap::new();
        data.insert("hello".into(), "Hello".into());
        data.insert("foo".into(), "Foo bar".into());
        backend.add_translations("en".into(), data);

        let mut data_cn = HashMap::new();
        data_cn.insert("hello".into(), "你好".into());
        data_cn.insert("foo".into(), "Foo 测试".into());
        backend.add_translations("zh-CN".into(), data_cn);

        assert_eq!(backend.translate("en", "hello"), Some(Cow::from("Hello")));
        assert_eq!(backend.translate("en", "foo"), Some(Cow::from("Foo bar")));
        assert_eq!(backend.translate("zh-CN", "hello"), Some(Cow::from("你好")));
        assert_eq!(
            backend.translate("zh-CN", "foo"),
            Some(Cow::from("Foo 测试"))
        );

        assert_eq!(backend.available_locales(), vec!["en", "zh-CN"]);
    }

    #[test]
    fn test_combined_backend() {
        let mut backend = SimpleBackend::new();
        let mut data = HashMap::new();
        data.insert("hello".into(), "Hello".into());
        data.insert("foo".into(), "Foo bar".into());
        backend.add_translations("en".into(), data);

        let mut data_cn = HashMap::new();
        data_cn.insert("hello".into(), "你好".into());
        data_cn.insert("foo".into(), "Foo 测试".into());
        backend.add_translations("zh-CN".into(), data_cn);

        let mut backend2 = SimpleBackend::new();
        let mut data2 = HashMap::new();
        data2.insert("hello".into(), "Hello2".into());
        backend2.add_translations("en".into(), data2);

        let mut data_cn2 = HashMap::new();
        data_cn2.insert("hello".into(), "你好2".into());
        backend2.add_translations("zh-CN".into(), data_cn2);

        let combined = backend.extend(backend2);
        assert_eq!(combined.translate("en", "hello"), Some(Cow::from("Hello2")));
        assert_eq!(
            combined.translate("zh-CN", "hello"),
            Some(Cow::from("你好2"))
        );

        assert_eq!(combined.available_locales(), vec!["en", "zh-CN"]);
    }

    #[test]
    fn test_namespaced_backend() {
        let mut backend = SimpleBackend::new();
        let mut data = HashMap::new();
        data.insert("ui_component.title".into(), "Custom title".into());
        data.insert("title".into(), "Unrelated title".into());
        backend.add_translations("en".into(), data);

        let backend = Box::leak(Box::new(backend));
        let namespaced = NamespacedBackend::new(backend, "ui_component");

        assert_eq!(
            namespaced.translate("en", "title"),
            Some(Cow::Borrowed("Custom title"))
        );
        assert_eq!(
            namespaced.messages_for_locale("en"),
            Some(vec![(
                Cow::Owned("title".to_string()),
                Cow::Borrowed("Custom title")
            )])
        );
    }
}
