#![doc = include_str!("../README.md")]

mod asm;

use std::{ops::Deref, sync::LazyLock};

#[doc(hidden)]
pub use rust_i18n_macro::{_minify_key, _tr, i18n};
#[cfg(feature = "load-path")]
pub use rust_i18n_support::try_load_locales;
pub use rust_i18n_support::{
    AtomicStr, Backend, BackendExt, CowStr, MinifyKey, NamespacedBackend, SimpleBackend,
    DEFAULT_MINIFY_KEY, DEFAULT_MINIFY_KEY_LEN, DEFAULT_MINIFY_KEY_PREFIX,
    DEFAULT_MINIFY_KEY_THRESH,
};

static CURRENT_LOCALE: LazyLock<AtomicStr> = LazyLock::new(|| AtomicStr::from("en"));

/// Set current locale
pub fn set_locale(locale: &str) {
    CURRENT_LOCALE.replace(locale);
}

/// Get current locale
#[inline]
pub fn locale() -> impl Deref<Target = str> {
    CURRENT_LOCALE.as_str()
}

/// Replace patterns and return a new string.
///
/// # Arguments
///
/// * `input` - The input string, containing patterns like `%{name}`.
/// * `patterns` - The patterns to replace.
/// * `values` - The values to replace.
///
/// # Example
///
/// ```
/// # use rust_i18n::replace_patterns;
/// let input = "Hello, %{name}!";
/// let patterns = &["name"];
/// let values = &["world".to_string()];
/// let output = replace_patterns(input, patterns, values);
/// assert_eq!(output, "Hello, world!");
/// ```
pub fn replace_patterns(input: &str, patterns: &[&str], values: &[String]) -> String {
    replace_patterns_impl(input, patterns, values)
}

/// Replace patterns using borrowed or owned values from generated macros.
#[doc(hidden)]
pub fn replace_patterns_cow(
    input: &str,
    patterns: &[&str],
    values: &[std::borrow::Cow<'_, str>],
) -> String {
    asm::interpolate::replace_patterns_cow(input, patterns, values)
}

fn replace_patterns_impl<V: AsRef<str>>(input: &str, patterns: &[&str], values: &[V]) -> String {
    replace_patterns_fast(input, patterns, values)
        .unwrap_or_else(|| replace_patterns_legacy(input, patterns, values))
}

// The common form has complete `%{name}` markers. Keep the original state
// machine for malformed input, whose historical behavior is more permissive.
fn replace_patterns_fast<V: AsRef<str>>(
    input: &str,
    patterns: &[&str],
    values: &[V],
) -> Option<String> {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len() + 128);
    let mut offset = 0;
    loop {
        let Some(percent) = asm::find_percent(&bytes[offset..]) else {
            output.extend_from_slice(&bytes[offset..]);
            // SAFETY: Each copied input slice begins or ends at an ASCII marker
            // boundary, so it remains valid UTF-8. Replacements are strings.
            return Some(unsafe { String::from_utf8_unchecked(output) });
        };
        let percent = offset + percent;
        if bytes.get(percent + 1) != Some(&b'{') {
            return None;
        }
        let start = percent + 2;
        let Some(end) = bytes[start..]
            .iter()
            .position(|&byte| byte == b'}' || byte == b'%')
        else {
            // An unfinished final marker is literal text in the legacy parser.
            output.extend_from_slice(&bytes[offset..]);
            // SAFETY: As above, all copied slices have UTF-8 boundaries.
            return Some(unsafe { String::from_utf8_unchecked(output) });
        };
        let end = end + start;
        if bytes[end] == b'%' {
            return None;
        }
        output.extend_from_slice(&bytes[offset..percent]);
        let key = &bytes[start..end];
        if let Some((_, value)) = patterns
            .iter()
            .zip(values)
            .find(|(&pattern, _)| pattern.as_bytes() == key)
        {
            output.extend_from_slice(value.as_ref().as_bytes());
        } else {
            output.extend_from_slice(&bytes[percent..=end]);
        }
        offset = end + 1;
    }
}

fn replace_patterns_legacy<V: AsRef<str>>(input: &str, patterns: &[&str], values: &[V]) -> String {
    let input_bytes = input.as_bytes();
    let mut pattern_pos = smallvec::SmallVec::<[usize; 64]>::new();
    let mut stage = 0;
    for (i, &b) in input_bytes.iter().enumerate() {
        match (stage, b) {
            (1, b'{') => {
                stage = 2;
                pattern_pos.push(i);
            }
            (2, b'}') => {
                stage = 0;
                pattern_pos.push(i);
            }
            (_, b'%') => {
                stage = 1;
            }
            _ => {}
        }
    }
    let mut output: Vec<u8> = Vec::with_capacity(input_bytes.len() + 128);
    let mut prev_end = 0;
    let pattern_values = patterns.iter().zip(values.iter());
    for pos in pattern_pos.chunks_exact(2) {
        let start = pos[0];
        let end = pos[1];
        let key = &input_bytes[start + 1..end];
        if prev_end < start {
            let prev_chunk = &input_bytes[prev_end..start - 1];
            output.extend_from_slice(prev_chunk);
        }
        if let Some((_, v)) = pattern_values
            .clone()
            .find(|(&pattern, _)| pattern.as_bytes() == key)
        {
            output.extend_from_slice(v.as_ref().as_bytes());
        } else {
            output.extend_from_slice(&input_bytes[start - 1..end + 1]);
        }
        prev_end = end + 1;
    }
    if prev_end < input_bytes.len() {
        let remaining = &input_bytes[prev_end..];
        output.extend_from_slice(remaining);
    }
    unsafe { String::from_utf8_unchecked(output) }
}

#[cfg(test)]
mod replace_patterns_tests {
    use super::{replace_patterns, replace_patterns_cow, replace_patterns_legacy};
    use std::borrow::Cow;

    #[test]
    fn replaces_multiple_placeholders_without_changing_surrounding_unicode() {
        let values = ["Jason".to_string(), "世界".to_string()];
        assert_eq!(
            replace_patterns(
                "你好，%{name}！Welcome to %{place}.",
                &["name", "place"],
                &values
            ),
            "你好，Jason！Welcome to 世界."
        );
    }

    #[test]
    fn preserves_unknown_and_incomplete_placeholders() {
        let values = ["Jason".to_string()];
        assert_eq!(
            replace_patterns("%{unknown} %{name} %{unfinished", &["name"], &values),
            "%{unknown} Jason %{unfinished"
        );
    }

    #[test]
    fn long_input_uses_the_same_replacements() {
        let input = format!(
            "{}%{{name}}{}%{{missing}}",
            "界".repeat(128),
            "a".repeat(512)
        );
        let values = ["世界".to_string()];
        assert_eq!(
            replace_patterns(&input, &["name"], &values),
            replace_patterns_legacy(&input, &["name"], &values)
        );
    }

    #[test]
    fn cow_values_match_string_values_for_complete_and_malformed_inputs() {
        let string_values = ["Jason".to_string(), "世界".to_string()];
        let cow_values = [Cow::Borrowed("Jason"), Cow::Owned("世界".to_string())];
        let patterns = ["name", "place"];
        let long_input = format!("{}%{{name}}{}%{{place}}", "界".repeat(128), "a".repeat(512));

        for input in [
            "你好，%{name}！Welcome to %{place}.",
            "%{unknown} %{name} %{unfinished",
            "%{name}%{place}",
            "plain text",
            "unexpected % marker",
            "%{name%{place}",
            long_input.as_str(),
        ] {
            assert_eq!(
                replace_patterns_cow(input, &patterns, &cow_values),
                replace_patterns(input, &patterns, &string_values),
                "input: {input:?}"
            );
        }
    }

    #[test]
    fn fast_path_matches_legacy_for_short_marker_sequences() {
        const ALPHABET: &[u8] = b"%{}ax";
        let values = ["世界".to_string(), "%{".to_string()];
        let cow_values = [Cow::Borrowed("世界"), Cow::Owned("%{".to_string())];
        for len in 0..=7_u32 {
            for mut code in 0..ALPHABET.len().pow(len) {
                let mut input = String::new();
                for _ in 0..len {
                    input.push(ALPHABET[code % ALPHABET.len()] as char);
                    code /= ALPHABET.len();
                }
                assert_eq!(
                    replace_patterns(&input, &["a", "x"], &values),
                    replace_patterns_legacy(&input, &["a", "x"], &values),
                    "input: {input:?}"
                );
                assert_eq!(
                    replace_patterns_cow(&input, &["a", "x"], &cow_values),
                    replace_patterns(&input, &["a", "x"], &values),
                    "input: {input:?}"
                );
            }
        }
    }
}

/// Get I18n text
///
/// This macro forwards to the `crate::_rust_i18n_t!` macro, which is generated by the [`i18n!`] macro.
///
/// # Arguments
///
/// * `expr` - The key or message for translation.
///   - A key usually looks like `"foo.bar.baz"`.
///   - A literal message usually looks like `"Hello, world!"`.
///   - The variable names in the message should be wrapped in `%{}`, like `"Hello, %{name}!"`.
///   - Dynamic messages are also supported, such as `t!(format!("Hello, {}!", name))`.
///     However, if `minify_key` is enabled, the entire message will be hashed and used as a key for every lookup, which may consume more CPU cycles.
/// * `locale` - The locale to use. If not specified, the current locale will be used.
/// * `args` - The arguments to be replaced in the translated text.
///    - These should be passed in the format `key = value` or `key => value`.
///    - Alternatively, you can specify the value format using the `key = value : {:format_specifier}` syntax.
///      For example, `key = value : {:08}` will format the value as a zero-padded string with a length of 8.
///
/// # Example
///
/// ```no_run
/// #[macro_use] extern crate rust_i18n;
///
/// # macro_rules! t { ($($all:tt)*) => {} }
/// # fn main() {
/// // Simple get text with current locale
/// t!("greeting");
/// // greeting: "Hello world" => "Hello world"
///
/// // Get a special locale's text
/// t!("greeting", locale = "de");
/// // greeting: "Hallo Welt!" => "Hallo Welt!"
///
/// // With variables
/// t!("messages.hello", name = "world");
/// // messages.hello: "Hello, %{name}" => "Hello, world"
/// t!("messages.foo", name = "Foo", other ="Bar");
/// // messages.foo: "Hello, %{name} and %{other}" => "Hello, Foo and Bar"
///
/// // With variables and format specifiers
/// t!("Hello, %{name}, you serial number is: %{sn}", name = "Jason", sn = 123 : {:08});
/// // => "Hello, Jason, you serial number is: 000000123"
///
/// // With locale and variables
/// t!("messages.hello", locale = "de", name = "Jason");
/// // messages.hello: "Hallo, %{name}" => "Hallo, Jason"
/// # }
/// ```
#[macro_export]
#[allow(clippy::crate_in_macro_def)]
macro_rules! t {
    ($($all:tt)*) => {
        crate::_rust_i18n_t!($($all)*)
    }
}

/// A macro that generates a translation key and corresponding value pair from a given input value.
///
/// It's useful when you want to use a long string as a key, but you don't want to type it twice.
///
/// # Arguments
///
/// * `msg` - The input value.
///
/// # Returns
///
/// A tuple of `(key, msg)`.
///
/// # Example
///
/// ```no_run
/// use rust_i18n::{t, tkv};
///
/// # macro_rules! t { ($($all:tt)*) => { } }
/// # macro_rules! tkv { ($($all:tt)*) => { (1,2) } }
///
/// let (key, msg) = tkv!("Hello world");
/// // => key is `"Hello world"` and msg is the translated message.
/// // => If there is hints the minify_key logic, the key will returns a minify key.
/// ```
#[macro_export]
#[allow(clippy::crate_in_macro_def)]
macro_rules! tkv {
    ($msg:literal) => {
        crate::_rust_i18n_tkv!($msg)
    };
}

/// Get available locales
///
/// ```no_run
/// #[macro_use] extern crate rust_i18n;
/// # pub fn _rust_i18n_available_locales() -> Vec<&'static str> { todo!() }
/// # fn main() {
/// rust_i18n::available_locales!();
/// # }
/// // => ["en", "zh-CN"]
/// ```
#[macro_export(local_inner_macros)]
#[allow(clippy::crate_in_macro_def)]
macro_rules! available_locales {
    () => {
        crate::_rust_i18n_available_locales()
    };
}

/// Extend a dependency's translations with the matching crate namespace from
/// the current crate's backend.
///
/// Given `extend!(ui_component)`, translations below the `ui_component` key in
/// the current crate are lazily merged with translations in the `ui_component`
/// crate. The extension takes priority for the same locale and key.
///
/// The lookup order for each locale and key is equivalent to:
///
/// ```rs, ignore
/// app.translate(key)
///     .or_else(|| ui_component.translate(key))
/// ```
///
/// If both miss, the existing locale fallback rules continue as usual.
#[macro_export]
macro_rules! extend {
    ($target:ident) => {
        $target::_rust_i18n_extend(crate::_rust_i18n_backend(), stringify!($target))
    };
}

#[cfg(test)]
mod tests {
    use crate::{locale, CURRENT_LOCALE};

    fn assert_locale_type(s: &str, val: &str) {
        assert_eq!(s, val);
    }

    #[test]
    fn test_locale() {
        assert_locale_type(&locale(), &CURRENT_LOCALE.as_str());
        assert_eq!(&*locale(), "en");
    }
}
