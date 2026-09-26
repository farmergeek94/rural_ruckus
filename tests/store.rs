//! The store on disk, in the build's own temporary folder.

use std::path::PathBuf;

use monster_truck_rural_ruckus::store::Store;

fn scratch(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_file(&path);
    path
}

#[test]
fn what_was_written_is_there_when_the_file_is_opened_again() {
    let path = scratch("store_reopened/ruckus.redb");
    {
        let store = Store::open(&path).expect("a new store, in a new folder");
        store
            .write(|batch| {
                batch
                    .set_text("choice.track", "builtin")
                    .set_number("setup.suspension", -0.5)
                    .set_blob("world.quick", b"later");
            })
            .unwrap();
    }
    let store = Store::open(&path).unwrap();
    assert_eq!(
        store.get_text("choice.track").unwrap().as_deref(),
        Some("builtin")
    );
    assert_eq!(store.get_number("setup.suspension").unwrap(), Some(-0.5));
    assert_eq!(
        store.get_blob("world.quick").unwrap().as_deref(),
        Some(&b"later"[..])
    );
}

#[test]
fn a_file_that_is_not_a_store_is_an_error_that_names_it() {
    let path = scratch("not_a_store.redb");
    std::fs::write(&path, b"This is not a database, however it is named.").unwrap();
    let Err(error) = Store::open(&path) else {
        panic!("opened a text file as a store");
    };
    assert!(error.to_string().contains("not_a_store.redb"), "{error}");
    // And it was left as it was found.
    assert!(std::fs::read(&path).unwrap().starts_with(b"This is not"));
}
