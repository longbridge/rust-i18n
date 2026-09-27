//! Small executable for comparing linked release binary size across revisions.

use rust_i18n::t;

rust_i18n::i18n!("examples/app/locales");

fn main() {
    let mut args = std::env::args().skip(1);
    let locale = args.next().unwrap_or_else(|| "en".to_string());
    let name = args.next().unwrap_or_else(|| "Jason".to_string());

    rust_i18n::set_locale(&locale);
    println!("{}", t!("view.buttons.ok"));
    println!("{}", t!("hello", name = name.as_str()));
    println!(
        "{}",
        t!("hello", locale = locale.as_str(), name = name.as_str())
    );
}
