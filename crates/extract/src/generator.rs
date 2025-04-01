use crate::extractor::Message;
use rust_i18n_support::load_locales;
use std::collections::HashMap;
use std::io::prelude::*;
use std::io::Result;
use std::path::Path;

type Translations = HashMap<String, HashMap<String, String>>;

pub fn generate<'a, P: AsRef<Path>>(
    output_path: P,
    all_locales: &Vec<String>,
    messages: impl IntoIterator<Item = (&'a String, &'a Message)> + Clone,
) -> Result<()> {
    let filename = "TODO.yml";
    let format = "yaml";

    let trs = generate_result(&output_path, filename, all_locales, messages);

    if trs.is_empty() {
        println!("All thing done.\n");

        return Ok(());
    }

    eprintln!("Found {} new texts need to translate.", trs.len());
    eprintln!("----------------------------------------");
    eprintln!("Writing to {}\n", filename);

    let text = convert_text(&trs, format);
    write_file(&output_path, filename, &text)?;

    // Finally, return error for let CI fail
    let err = std::io::Error::new(std::io::ErrorKind::Other, "");
    Err(err)
}

fn convert_text(trs: &Translations, format: &str) -> String {
    let mut value = serde_json::Value::Object(serde_json::Map::new());
    value["_version"] = serde_json::Value::Number(serde_json::Number::from(2));

    for (key, val) in trs {
        let mut obj = serde_json::Value::Object(serde_json::Map::new());
        for (locale, text) in val {
            obj[locale] = serde_json::Value::String(text.clone());
        }
        value[key] = obj;
    }

    match format {
        "json" => serde_json::to_string_pretty(&value).unwrap(),
        "yaml" | "yml" => {
            let text = serde_yaml::to_string(&value).unwrap();
            // Remove leading `---`
            text.trim_start_matches("---").trim_start().to_string()
        }
        "toml" => toml::to_string_pretty(&value).unwrap(),
        _ => unreachable!(),
    }
}

fn generate_result<'a, P: AsRef<Path>>(
    output_path: P,
    output_filename: &str,
    all_locales: &Vec<String>,
    messages: impl IntoIterator<Item = (&'a String, &'a Message)> + Clone,
) -> Translations {
    let mut trs = Translations::new();

    for locale in all_locales {
        println!("Checking [{}] and generating untranslated texts...", locale);

        // ~/work/my-project/locales
        let output_path = output_path.as_ref().display().to_string();

        let ignore_file = |fname: &str| fname.ends_with(&output_filename);
        let data = load_locales(&output_path, ignore_file);

        for (key, m) in messages.clone() {
            if !m.locations.is_empty() {
                for _l in &m.locations {
                    // TODO: write file and line as YAML comment
                    // Reason: serde_yaml not support write comments
                    // https://github.com/dtolnay/serde-yaml/issues/145
                }
            }

            if let Some(trs) = data.get(locale) {
                if trs.get(key).is_some() {
                    continue;
                }
            }

            let value = if m.minify_key {
                m.key.to_owned()
            } else {
                m.key.split('.').last().unwrap_or_default().to_string()
            };

            trs.entry(key.clone())
                .or_default()
                .insert(locale.to_string(), value.to_string());
        }
    }

    trs
}

fn write_file<P: AsRef<Path>>(output: &P, filename: &str, data: &str) -> Result<()> {
    let output_file = std::path::Path::new(output.as_ref()).join(String::from(filename));
    let folder = output_file.parent().unwrap();

    // Ensure create folder
    if !folder.exists() {
        std::fs::create_dir_all(folder).unwrap();
    }

    let mut output = ::std::fs::File::create(&output_file)
        .unwrap_or_else(|_| panic!("Unable to create {} file", &output_file.display()));

    writeln!(output, "{}", data).expect("Write YAML file error");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    fn assert_eq_json(left: &str, right: &str) {
        let left: serde_json::Value = serde_json::from_str(left).unwrap();
        let right: serde_json::Value = serde_json::from_str(right).unwrap();
        assert_eq!(left, right);
    }

    #[test]
    fn test_convert_text() {
        let mut trs = Translations::new();
        let format = "json";

        let result = convert_text(&trs, format);
        let expect = r#"
        {
            "_version": 2
        }
        "#;
        assert_eq_json(&result, expect);

        trs.insert("hello".to_string(), {
            let mut map = HashMap::new();
            map.insert("en".to_string(), "Hello".to_string());
            map.insert("zh".to_string(), "你好".to_string());
            map
        });

        let result = convert_text(&trs, format);
        let expect = r#"
        {
            "_version": 2,
            "hello": {
                "en": "Hello",
                "zh": "你好"
            }
        }
        "#;
        assert_eq_json(&result, expect);

        let format = "yaml";
        let result = convert_text(&trs, format);
        let expect = indoc! {r#"
        _version: 2
        hello:
          en: Hello
          zh: 你好
        "#};
        assert_eq!(&result, &expect);

        let format = "toml";
        let result = convert_text(&trs, format);
        let expect = indoc! {r#"
        _version = 2

        [hello]
        en = "Hello"
        zh = "你好"
        "#};
        assert_eq!(&result, &expect);
    }
}
