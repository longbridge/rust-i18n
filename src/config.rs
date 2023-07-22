//! This crate defines `struct`s that can be deserialized with Serde
//! to load and inspect `Cargo.toml` metadata.
//!
//! See `Manifest::from_slice`.

use itertools::Itertools;
use std::fs;
use std::io;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct I18nConfig {
    #[serde(default = "default_locale")]
    pub default_locale: String,
    #[serde(default = "available_locales")]
    pub available_locales: Vec<String>,
    #[serde(default = "load_path")]
    pub load_path: String,
}

fn default_locale() -> String {
    I18nConfig::default().default_locale
}

fn available_locales() -> Vec<String> {
    I18nConfig::default().available_locales
}

fn load_path() -> String {
    I18nConfig::default().load_path
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct MainConfig {
    pub i18n: I18nConfig,
}

impl Default for I18nConfig {
    fn default() -> Self {
        I18nConfig {
            default_locale: "en".to_string(),
            available_locales: vec!["en".to_string()],
            load_path: "./locales".to_string(),
        }
    }
}

pub fn load(cargo_root: &Path) -> io::Result<I18nConfig> {
    let cargo_file = cargo_root.join("Cargo.toml");
    let mut file = fs::File::open(&cargo_file)
        .unwrap_or_else(|e| panic!("Fail to open {}, {}", cargo_file.display(), e));

    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    parse(&contents)
}

pub fn parse(contents: &str) -> io::Result<I18nConfig> {
    if !contents.contains("[i18n]") && !contents.contains("[package.metadata.i18n]") {
        return Ok(I18nConfig::default());
    }
    let contents = contents.replace("[package.metadata.i18n]", "[i18n]");
    let mut config: MainConfig = toml::from_str(&contents)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    // Push default_locale
    config
        .i18n
        .available_locales
        .insert(0, config.i18n.default_locale.clone());

    // unqiue
    config.i18n.available_locales = config.i18n.available_locales.into_iter().unique().collect();

    Ok(config.i18n)
}

#[test]
fn test_parse() {
    let contents = r#"
        [i18n]
        default-locale = "en"
        available-locales = ["zh-CN"]
        load-path = "./my-locales"
    "#;

    let cfg = parse(contents).unwrap();
    assert_eq!(cfg.default_locale, "en");
    assert_eq!(cfg.available_locales, vec!["en", "zh-CN"]);
    assert_eq!(cfg.load_path, "./my-locales");

    let contents = r#"
        [i18n]
        available-locales = ["zh-CN", "de", "de"]
        load-path = "./my-locales"
    "#;
    let cfg = parse(contents).unwrap();
    assert_eq!(cfg.default_locale, "en");
    assert_eq!(cfg.available_locales, vec!["en", "zh-CN", "de"]);
    assert_eq!(cfg.load_path, "./my-locales");

    let contents = "";
    let cfg = parse(contents).unwrap();
    assert_eq!(cfg.default_locale, "en");
    assert_eq!(cfg.available_locales, vec!["en"]);
    assert_eq!(cfg.load_path, "./locales");
}

#[test]
fn test_parse_with_metadata() {
    let contents = r#"
        [package.metadata.i18n]
        default-locale = "en"
        available-locales = ["zh-CN"]
        load-path = "./my-locales"
    "#;

    let cfg = parse(contents).unwrap();
    assert_eq!(cfg.default_locale, "en");
    assert_eq!(cfg.available_locales, vec!["en", "zh-CN"]);
    assert_eq!(cfg.load_path, "./my-locales");
}

#[test]
fn test_load_default() {
    let workdir = Path::new(env!["CARGO_MANIFEST_DIR"]);

    let cfg = load(workdir).unwrap();
    assert_eq!(cfg.default_locale, "en");
    assert_eq!(cfg.available_locales, vec!["en"]);
    assert_eq!(cfg.load_path, "./locales");
}

#[test]
fn test_load() {
    let workdir = Path::new(env!["CARGO_MANIFEST_DIR"]);
    let cargo_root = workdir.join("examples/foo");

    let cfg = load(&cargo_root).unwrap();
    assert_eq!(cfg.default_locale, "en");
    assert_eq!(cfg.available_locales, vec!["en", "zh-CN"]);
}
