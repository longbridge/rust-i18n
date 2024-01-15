use quote::quote;
use rust_i18n_support::{is_debug, load_locales};
use std::collections::HashMap;
use syn::{parse_macro_input, Expr, Ident, LitStr, Token};

struct Args {
    locales_path: String,
    fallback: Option<String>,
    extend: Option<Expr>,
}

impl Args {
    fn consume_path(&mut self, input: syn::parse::ParseStream) -> syn::parse::Result<()> {
        let path = input.parse::<LitStr>()?;
        self.locales_path = path.value();

        Ok(())
    }

    fn consume_options(&mut self, input: syn::parse::ParseStream) -> syn::parse::Result<()> {
        let ident = input.parse::<Ident>()?.to_string();
        input.parse::<Token![=]>()?;

        match ident.as_str() {
            "fallback" => {
                let val = input.parse::<LitStr>()?.value();
                self.fallback = Some(val);
            }
            "backend" => {
                let val = input.parse::<Expr>()?;
                self.extend = Some(val);
            }
            _ => {}
        }

        // Continue to consume reset of options
        if input.parse::<Token![,]>().is_ok() {
            self.consume_options(input)?;
        }

        Ok(())
    }
}

impl syn::parse::Parse for Args {
    /// Parse macro arguments.
    ///
    /// ```no_run
    /// # use rust_i18n::i18n;
    /// # fn v1() {
    /// i18n!();
    /// # }
    /// # fn v2() {
    /// i18n!("locales");
    /// # }
    /// # fn v3() {
    /// i18n!("locales", fallback = "en");
    /// # }
    /// ```
    ///
    /// Ref: https://docs.rs/syn/latest/syn/parse/index.html
    fn parse(input: syn::parse::ParseStream) -> syn::parse::Result<Self> {
        let lookahead = input.lookahead1();

        let mut result = Self {
            locales_path: String::from("locales"),
            fallback: None,
            extend: None,
        };

        if lookahead.peek(LitStr) {
            result.consume_path(input)?;

            if input.parse::<Token![,]>().is_ok() {
                result.consume_options(input)?;
            }
        } else if lookahead.peek(Ident) {
            result.consume_options(input)?;
        }

        Ok(result)
    }
}

/// Init I18n translations.
///
/// This will load all translations by glob `**/*.yml` from the given path, default: `${CARGO_MANIFEST_DIR}/locales`.
///
/// Attribute `fallback` for set the fallback locale, if present `t` macro will use it as the fallback locale.
///
/// ```no_run
/// # use rust_i18n::i18n;
/// # fn v1() {
/// i18n!();
/// # }
/// # fn v2() {
/// i18n!("locales");
/// # }
/// # fn v3() {
/// i18n!("locales", fallback = "en");
/// # }
/// ```
#[proc_macro]
pub fn i18n(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let args = parse_macro_input!(input as Args);

    // CARGO_MANIFEST_DIR is current build directory
    let cargo_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is empty");
    let current_dir = std::path::PathBuf::from(cargo_dir);
    let locales_path = current_dir.join(&args.locales_path);

    let data = load_locales(&locales_path.display().to_string(), |_| false);
    let code = generate_code(data, args);

    if is_debug() {
        println!(
            "\n\n-------------- code --------------\n{}\n----------------------------------\n\n",
            code
        );
    }

    code.into()
}

fn generate_code(
    translations: HashMap<String, HashMap<String, String>>,
    args: Args,
) -> proc_macro2::TokenStream {
    let mut all_translations = Vec::<proc_macro2::TokenStream>::new();

    translations.iter().for_each(|(locale, trs)| {
        let mut sub_trs = Vec::<proc_macro2::TokenStream>::new();

        trs.iter().for_each(|(k, v)| {
            let k = k.to_string();
            let v = v.to_string();
            sub_trs.push(quote! {
                (#k, #v)
            });
        });

        all_translations.push(quote! {
            let trs = [#(#sub_trs),*];
            backend.add_translations(#locale, &trs.into_iter().collect());
        });
    });

    let fallback = if let Some(fallback) = args.fallback {
        quote! {
            Some(#fallback)
        }
    } else {
        quote! {
            None
        }
    };

    let extend_code = if let Some(extend) = args.extend {
        quote! {
            let backend = backend.extend(#extend);
        }
    } else {
        quote! {}
    };

    // result
    quote! {
        use rust_i18n::BackendExt;

        /// I18n backend instance
        ///
        /// [PUBLIC] This is a public API, and as an example in examples/
        #[allow(missing_docs)]
        static _RUST_I18N_BACKEND: rust_i18n::once_cell::sync::Lazy<Box<dyn rust_i18n::Backend>> = rust_i18n::once_cell::sync::Lazy::new(|| {
            let mut backend = rust_i18n::SimpleBackend::new();
            #(#all_translations)*
            #extend_code

            Box::new(backend)
        });

        static _RUST_I18N_FALLBACK_LOCALE: Option<&'static str> = #fallback;

        /// Lookup fallback locales
        ///
        /// For example: `"zh-Hant-CN-x-private1-private2"` -> `"zh-Hant-CN-x-private1"` -> `"zh-Hant-CN"` -> `"zh-Hant"` -> `"zh"`.
        ///
        /// https://datatracker.ietf.org/doc/html/rfc4647#section-3.4
        #[inline]
        #[allow(missing_docs)]
        pub fn _rust_i18n_lookup_fallback(locale: &str) -> Option<&str> {
            locale.rfind('-').map(|n| locale[..n].trim_end_matches("-x"))
        }

        /// Get I18n text by locale and key
        #[inline]
        #[allow(missing_docs)]
        pub fn _rust_i18n_translate(locale: &str, key: &str) -> String {
            if let Some(value) = _RUST_I18N_BACKEND.translate(locale, key) {
                return value.to_string();
            }

            let mut current_locale = locale;
            while let Some(fallback_locale) = _rust_i18n_lookup_fallback(current_locale) {
                if let Some(value) = _RUST_I18N_BACKEND.translate(fallback_locale, key) {
                    return value.to_string();
                }
                current_locale = fallback_locale;
            }

            if let Some(fallback) = _RUST_I18N_FALLBACK_LOCALE {
                if let Some(value) = _RUST_I18N_BACKEND.translate(fallback, key) {
                    return value.to_string();
                }
            }

            if locale.is_empty() {
                return key.to_string();
            }
            return format!("{}.{}", locale, key);
        }

        #[allow(missing_docs)]
        pub fn _rust_i18n_available_locales() -> Vec<&'static str> {
            let mut locales = _RUST_I18N_BACKEND.available_locales();
            locales.sort();
            locales
        }
    }
}
