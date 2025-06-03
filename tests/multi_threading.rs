use rust_i18n::{set_locale, t};
use std::ops::Add;
use std::thread::spawn;
use std::time::{Duration, Instant};

rust_i18n::i18n!("locales", fallback = "en");

#[test]
fn test_load_and_store() {
    let end = Instant::now().add(Duration::from_secs(3));
    let store = spawn(move || {
        let mut i = 0u32;
        while Instant::now() < end {
            for _ in 0..100 {
                i = i.wrapping_add(1);
                if i % 2 == 0 {
                    set_locale(&format!("en-{i}"));
                } else {
                    set_locale(&format!("fr-{i}"));
                }
            }
        }
    });
    let load = spawn(move || {
        while Instant::now() < end {
            for _ in 0..100 {
                t!("hello");
            }
        }
    });
    store.join().unwrap();
    load.join().unwrap();
}

#[test]
fn test_t_concurrent() {
    let end = Instant::now().add(Duration::from_secs(3));
    let store = spawn(move || {
        let mut i = 0u32;
        while Instant::now() < end {
            for _ in 0..100 {
                i = i.wrapping_add(1);
                if i % 2 == 0 {
                    set_locale(&format!("en-{i}"));
                } else {
                    set_locale(&format!("fr-{i}"));
                }
            }
        }
    });
    let tasks: Vec<_> = (0..4)
        .map(|_| {
            spawn(move || {
                let locales = rust_i18n::available_locales!();
                let num_locales = locales.len();
                while Instant::now() < end {
                    for i in 0..100usize {
                        let m = i.checked_rem(num_locales).unwrap_or_default();
                        if m == 0 {
                            t!("hello");
                        } else {
                            t!("hello", locale = &locales[m]);
                        }
                    }
                }
            })
        })
        .collect();
    store.join().unwrap();
    for task in tasks {
        task.join().unwrap();
    }
}
