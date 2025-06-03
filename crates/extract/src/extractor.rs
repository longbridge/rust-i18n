use anyhow::Error;
use proc_macro2::{TokenStream, TokenTree};
use quote::ToTokens;
use rust_i18n_support::{DeterministicHashMap, I18nConfig};
use std::path::PathBuf;

pub type Results = DeterministicHashMap<String, Message>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub file: std::path::PathBuf,
    pub line: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Message {
    pub key: String,
    pub index: usize,
    pub minify_key: bool,
    pub locations: Vec<Location>,
}

impl Message {
    fn new(key: &str, index: usize, minify_key: bool) -> Self {
        Self {
            key: key.to_owned(),
            index,
            minify_key,
            locations: vec![],
        }
    }
}

static METHOD_NAMES: &[&str] = &["t", "tr"];

#[allow(clippy::ptr_arg)]
pub fn extract(
    results: &mut Results,
    path: &PathBuf,
    source: &str,
    cfg: I18nConfig,
) -> Result<(), Error> {
    let mut ex = Extractor { results, path, cfg };

    let file = syn::parse_file(source)
        .unwrap_or_else(|_| panic!("Failed to parse file, file: {}", path.display()));
    let stream = file.into_token_stream();
    ex.invoke(stream)
}

#[allow(dead_code)]
struct Extractor<'a> {
    results: &'a mut Results,
    path: &'a PathBuf,
    cfg: I18nConfig,
}

impl<'a> Extractor<'a> {
    fn invoke(&mut self, stream: TokenStream) -> Result<(), Error> {
        let mut token_iter = stream.into_iter().peekable();

        while let Some(token) = token_iter.next() {
            match token {
                TokenTree::Group(group) => self.invoke(group.stream())?,
                TokenTree::Ident(ident) => {
                    let mut is_macro = false;
                    if let Some(TokenTree::Punct(punct)) = token_iter.peek() {
                        if punct.to_string() == "!" {
                            is_macro = true;
                            token_iter.next();
                        }
                    }

                    let ident_str = ident.to_string();
                    if METHOD_NAMES.contains(&ident_str.as_str()) && is_macro {
                        if let Some(TokenTree::Group(group)) = token_iter.peek() {
                            self.take_message(group.stream());
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn take_message(&mut self, stream: TokenStream) {
        let mut token_iter = stream.into_iter().peekable();

        let literal = if let Some(TokenTree::Literal(literal)) = token_iter.next() {
            literal
        } else {
            return;
        };

        let I18nConfig {
            minify_key,
            minify_key_len,
            minify_key_prefix,
            minify_key_thresh,
            ..
        } = &self.cfg;
        let key: Option<proc_macro2::Literal> = Some(literal);

        if let Some(lit) = key {
            if let Some(key) = literal_to_string(&lit) {
                let (message_key, message_content) = if *minify_key {
                    let hashed_key = rust_i18n_support::MinifyKey::minify_key(
                        &key,
                        *minify_key_len,
                        minify_key_prefix,
                        *minify_key_thresh,
                    );
                    (hashed_key.to_string(), key.clone())
                } else {
                    let message_key = format_message_key(&key);
                    (message_key.clone(), message_key)
                };
                let index = self.results.len();
                let message = self
                    .results
                    .entry(message_key)
                    .or_insert_with(|| Message::new(&message_content, index, *minify_key));

                let span = lit.span();
                let line = span.start().line;
                if line > 0 {
                    message.locations.push(Location {
                        file: self.path.clone(),
                        line,
                    });
                }
            }
        }
    }
}

fn literal_to_string(lit: &proc_macro2::Literal) -> Option<String> {
    match syn::parse_str::<syn::LitStr>(&lit.to_string()) {
        Ok(lit) => Some(lit.value()),
        Err(_) => None,
    }
}

fn format_message_key(key: &str) -> String {
    let re = regex::Regex::new(r"\s+").unwrap();
    let key = re.replace_all(key, " ").into_owned();
    key.trim().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    macro_rules! build_messages {
        {$(($key:tt, $($line:tt),+)),+} => {{
            let mut results = Vec::<Message>::new();
            $(
                let message = Message {
                    key: $key.into(),
                    locations: vec![
                        $(
                            Location {
                                file: PathBuf::from_str("hello.rs").unwrap(),
                                line: $line
                            },
                        )+
                    ],
                    index: 0,
                    minify_key: false,
                };
                results.push(message);
            )+

            results
        }}
    }

    #[test]
    fn test_format_message_key() {
        assert_eq!(format_message_key("Hello world"), "Hello world".to_owned());
        assert_eq!(format_message_key("\n    "), "".to_owned());
        assert_eq!(format_message_key("\n    "), "".to_owned());
        assert_eq!(format_message_key("\n    hello"), "hello".to_owned());
        assert_eq!(format_message_key("\n    hello\n"), "hello".to_owned());
        assert_eq!(format_message_key("\n    hello\n    "), "hello".to_owned());
        assert_eq!(
            format_message_key("\n    hello\n    world"),
            "hello world".to_owned()
        );
        assert_eq!(
            format_message_key("\n    hello\n    world\n\n"),
            "hello world".to_owned()
        );
        assert_eq!(
            format_message_key("\n    hello\n    world\n    "),
            "hello world".to_owned()
        );
        assert_eq!(
            format_message_key("    hello\n    world\n    "),
            "hello world".to_owned()
        );
        assert_eq!(
            format_message_key(
                r#"Use YAML for mapping localized text, 
            and support mutiple YAML files merging."#
            ),
            "Use YAML for mapping localized text, and support mutiple YAML files merging."
                .to_owned()
        );
    }

    #[test]
    fn test_extract() {
        let source = include_str!("example.test.rs");
        let stream = proc_macro2::TokenStream::from_str(source).unwrap();

        let expected = build_messages![
            ("hello", 4),
            ("views.message.title", 5),
            ("views.message.description", 7),
            (
                "Use YAML for mapping localized text, and support mutiple YAML files merging.",
                11,
                14
            ),
            (
                "The table below describes some of those behaviours.",
                18,
                20
            )
        ];

        let mut results = DeterministicHashMap::default();

        let mut ex = Extractor {
            results: &mut results,
            path: &"hello.rs".to_owned().into(),
            cfg: I18nConfig::default(),
        };

        ex.invoke(stream).unwrap();

        let mut messages: Vec<_> = ex.results.values().collect();
        messages.sort_by_key(|m| m.index);
        assert_eq!(expected.len(), messages.len());

        for (expected_message, actually_message) in expected.iter().zip(messages) {
            let mut actually_message = actually_message.clone();
            actually_message.index = 0;

            assert_eq!(*expected_message, actually_message);
        }
    }
}
