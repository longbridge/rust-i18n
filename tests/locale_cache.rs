use rust_i18n::{locale, set_locale};
use std::sync::{Arc, Barrier};
use std::thread::spawn;

fn assert_send_sync<T: Send + Sync>(_: &T) {}

// One test so the global locale is not changed by parallel tests.
#[test]
fn locale_reflects_every_set_locale() {
    assert_send_sync(&locale());
    assert_eq!(&*locale(), "en");

    // The calling thread sees its own change immediately.
    set_locale("zh-CN");
    assert_eq!(&*locale(), "zh-CN");

    // A locale longer than any inline buffer.
    let long = "x-private-".repeat(8);
    set_locale(&long);
    assert_eq!(&*locale(), long);

    // A guard keeps the value it was created with.
    set_locale("fr");
    let held = locale();
    set_locale("de");
    assert_eq!(&*held, "fr");
    assert_eq!(&*locale(), "de");

    // A thread that already read the locale sees a later change.
    let read = Arc::new(Barrier::new(2));
    let changed = Arc::new(Barrier::new(2));
    let reader = spawn({
        let (read, changed) = (read.clone(), changed.clone());
        move || {
            let before = locale().to_string();
            read.wait();
            changed.wait();
            (before, locale().to_string())
        }
    });
    read.wait();
    set_locale("ja");
    changed.wait();
    assert_eq!(reader.join().unwrap(), ("de".to_string(), "ja".to_string()));

    // A new thread starts from the current value.
    assert_eq!(spawn(|| locale().to_string()).join().unwrap(), "ja");

    set_locale("en");
    assert_eq!(&*locale(), "en");
}
