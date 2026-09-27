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
    translations: LocaleTranslations,
}

type Messages = HashMap<Cow<'static, str>, Cow<'static, str>>;

// Comparing cached two-byte prefixes avoids hashing the locale before hashing
// the message key in small catalogs. Use a hash map for larger catalogs or
// many regional variants sharing a prefix, bounding full-string comparisons.
const SMALL_LOCALE_LIMIT: usize = 16;
const SMALL_PREFIX_LIMIT: usize = 4;

struct SmallLocale {
    prefix: u16,
    locale: Cow<'static, str>,
    messages: Messages,
}

impl SmallLocale {
    fn new(locale: Cow<'static, str>, messages: Messages) -> Self {
        Self {
            prefix: locale_prefix(&locale),
            locale,
            messages,
        }
    }
}

// A prefix is only an index. Empty, short, NUL-containing, and UTF-8 locale
// names can share it, so every candidate is also compared in full.
fn locale_prefix(locale: &str) -> u16 {
    let bytes = locale.as_bytes();
    u16::from_be_bytes([*bytes.first().unwrap_or(&0), *bytes.get(1).unwrap_or(&0)])
}

enum LocaleTranslations {
    Small(Vec<SmallLocale>),
    Large(HashMap<Cow<'static, str>, Messages>),
}

impl LocaleTranslations {
    fn from_map(map: HashMap<Cow<'static, str>, Messages>) -> Self {
        if map.len() <= SMALL_LOCALE_LIMIT {
            let mut entries = map
                .into_iter()
                .map(|(locale, messages)| SmallLocale::new(locale, messages))
                .collect::<Vec<_>>();
            entries.sort_by(|a, b| {
                a.prefix
                    .cmp(&b.prefix)
                    .then_with(|| a.locale.cmp(&b.locale))
            });
            if entries
                .windows(SMALL_PREFIX_LIMIT + 1)
                .any(|group| group.first().unwrap().prefix == group.last().unwrap().prefix)
            {
                return Self::Large(
                    entries
                        .into_iter()
                        .map(|entry| (entry.locale, entry.messages))
                        .collect(),
                );
            }
            Self::Small(entries)
        } else {
            Self::Large(map)
        }
    }

    fn get(&self, locale: &str) -> Option<&Messages> {
        match self {
            Self::Small(entries) => {
                let prefix = locale_prefix(locale);
                entries
                    .iter()
                    .find(|entry| entry.prefix == prefix && entry.locale.as_ref() == locale)
                    .map(|entry| &entry.messages)
            }
            Self::Large(entries) => entries.get(locale),
        }
    }

    fn add(&mut self, locale: Cow<'static, str>, data: Messages) {
        match self {
            Self::Small(entries) => {
                let prefix = locale_prefix(&locale);
                let start = entries.partition_point(|entry| entry.prefix < prefix);
                let existing = entries[start..]
                    .iter()
                    .take_while(|entry| entry.prefix == prefix)
                    .position(|entry| entry.locale.as_ref() == locale.as_ref());
                if let Some(index) = existing {
                    entries[start + index].messages.extend(data);
                } else if entries.len() < SMALL_LOCALE_LIMIT
                    && entries[start..]
                        .iter()
                        .take_while(|entry| entry.prefix == prefix)
                        .count()
                        < SMALL_PREFIX_LIMIT
                {
                    entries.insert(start, SmallLocale::new(locale, data));
                } else {
                    let mut map = std::mem::take(entries)
                        .into_iter()
                        .map(|entry| (entry.locale, entry.messages))
                        .collect::<HashMap<_, _>>();
                    map.insert(locale, data);
                    *self = Self::Large(map);
                }
            }
            Self::Large(entries) => entries.entry(locale).or_default().extend(data),
        }
    }

    fn available_locales(&self) -> Vec<Cow<'_, str>> {
        let mut locales: Vec<_> = match self {
            Self::Small(entries) => entries.iter().map(|entry| entry.locale.clone()).collect(),
            Self::Large(entries) => entries.keys().cloned().collect(),
        };
        locales.sort();
        locales
    }
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
            // HashMap::collect retains the existing last-wins behavior for
            // duplicate locale entries.
            translations: LocaleTranslations::from_map(iter.into_iter().collect()),
        }
    }
}

impl SimpleBackend {
    /// Create a new SimpleBackend.
    pub fn new() -> Self {
        SimpleBackend {
            translations: LocaleTranslations::Small(Vec::new()),
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
        self.translations.add(locale, data);
    }
}

impl Backend for SimpleBackend {
    fn available_locales(&self) -> Vec<Cow<'_, str>> {
        self.translations.available_locales()
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
    use super::{Backend, BackendExt, LocaleTranslations, NamespacedBackend, SMALL_LOCALE_LIMIT};

    #[test]
    fn small_locale_storage_transitions_and_keeps_merged_messages() {
        let mut backend = SimpleBackend::new();
        for index in 0..SMALL_LOCALE_LIMIT {
            let locale = format!("{index:02}-locale");
            backend.add_translations(
                Cow::Owned(locale),
                HashMap::from([(Cow::Borrowed("first"), Cow::Borrowed("value"))]),
            );
        }
        assert!(matches!(
            &backend.translations,
            LocaleTranslations::Small(_)
        ));

        backend.add_translations(
            Cow::Borrowed("00-locale"),
            HashMap::from([(Cow::Borrowed("second"), Cow::Borrowed("another"))]),
        );
        assert_eq!(
            backend.translate("00-locale", "first"),
            Some(Cow::Borrowed("value"))
        );
        assert_eq!(
            backend.translate("00-locale", "second"),
            Some(Cow::Borrowed("another"))
        );

        backend.add_translations(
            Cow::Borrowed("extra"),
            HashMap::from([(Cow::Borrowed("first"), Cow::Borrowed("extra value"))]),
        );
        assert!(matches!(
            &backend.translations,
            LocaleTranslations::Large(_)
        ));
        assert_eq!(
            backend.translate("00-locale", "second"),
            Some(Cow::Borrowed("another"))
        );
        assert_eq!(
            backend.translate("extra", "first"),
            Some(Cow::Borrowed("extra value"))
        );
        assert_eq!(backend.messages_for_locale("missing"), None);
        assert_eq!(backend.available_locales().len(), SMALL_LOCALE_LIMIT + 1);
    }

    #[test]
    fn from_iterator_duplicate_locale_uses_last_entry() {
        let first = HashMap::from([(Cow::Borrowed("key"), Cow::Borrowed("old"))]);
        let second = HashMap::from([(Cow::Borrowed("key"), Cow::Borrowed("new"))]);
        let backend =
            SimpleBackend::from_iter([(Cow::Borrowed("en"), first), (Cow::Borrowed("en"), second)]);

        assert!(matches!(
            &backend.translations,
            LocaleTranslations::Small(_)
        ));
        assert_eq!(backend.available_locales(), vec!["en"]);
        assert_eq!(backend.translate("en", "key"), Some(Cow::Borrowed("new")));
    }

    #[test]
    fn small_locale_index_checks_full_name_after_prefix_collision() {
        let mut backend = SimpleBackend::new();
        for (locale, value) in [
            ("", "empty"),
            ("\0", "nul"),
            ("a", "short"),
            ("a\0", "short nul"),
            ("zh", "chinese"),
            ("zh-CN", "simplified"),
            ("é", "accent"),
            ("é-x", "accent extended"),
        ] {
            backend.add_translations(
                Cow::Borrowed(locale),
                HashMap::from([(Cow::Borrowed("key"), Cow::Borrowed(value))]),
            );
        }

        assert!(matches!(
            &backend.translations,
            LocaleTranslations::Small(_)
        ));
        for (locale, expected) in [
            ("", "empty"),
            ("\0", "nul"),
            ("a", "short"),
            ("a\0", "short nul"),
            ("zh", "chinese"),
            ("zh-CN", "simplified"),
            ("é", "accent"),
            ("é-x", "accent extended"),
        ] {
            assert_eq!(
                backend.translate(locale, "key"),
                Some(Cow::Borrowed(expected))
            );
        }
        for missing in ["\0\0", "a\0x", "zh-TW", "é-y"] {
            assert_eq!(backend.translate(missing, "key"), None);
        }
    }

    #[test]
    fn adding_fifth_shared_prefix_promotes_to_hash_map() {
        let mut backend = SimpleBackend::new();
        for index in 0..4 {
            backend.add_translations(
                Cow::Owned(format!("en-{index}")),
                HashMap::from([(Cow::Borrowed("key"), Cow::Owned(index.to_string()))]),
            );
        }
        assert!(matches!(
            &backend.translations,
            LocaleTranslations::Small(_)
        ));

        backend.add_translations(
            Cow::Borrowed("en-0"),
            HashMap::from([(Cow::Borrowed("extra"), Cow::Borrowed("merged"))]),
        );
        assert!(matches!(
            &backend.translations,
            LocaleTranslations::Small(_)
        ));

        backend.add_translations(
            Cow::Borrowed("en-4"),
            HashMap::from([(Cow::Borrowed("key"), Cow::Borrowed("four"))]),
        );
        assert!(matches!(
            &backend.translations,
            LocaleTranslations::Large(_)
        ));
        assert_eq!(
            backend.translate("en-0", "extra"),
            Some(Cow::Borrowed("merged"))
        );
        assert_eq!(
            backend.translate("en-4", "key"),
            Some(Cow::Borrowed("four"))
        );
        assert_eq!(backend.translate("en-missing", "key"), None);
        assert_eq!(backend.available_locales().len(), 5);
    }

    #[test]
    fn from_iterator_with_five_shared_prefixes_uses_hash_map() {
        let backend = SimpleBackend::from_iter((0..5).map(|index| {
            (
                Cow::Owned(format!("en-{index}")),
                HashMap::from([(Cow::Borrowed("key"), Cow::Owned(index.to_string()))]),
            )
        }));

        assert!(matches!(
            &backend.translations,
            LocaleTranslations::Large(_)
        ));
        assert_eq!(backend.translate("en-4", "key"), Some(Cow::Borrowed("4")));
        assert_eq!(backend.translate("en-missing", "key"), None);
    }

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
