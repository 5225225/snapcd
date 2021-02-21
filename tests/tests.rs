use rand::prelude::*;
use rand_chacha::ChaChaRng;
use snapcd::file::{put_data, read_data};
use snapcd::{ds::sqlite::SqliteDs, DataStore};
use std::collections::HashSet;
use std::io::{Read, Write};

fn internal_test<T: DataStore, F: FnMut() -> T>(
    ctor: &mut F,
    size_upper_bound: usize,
    seed_lower_bound: u64,
    seed_upper_bound: u64,
) {
    for i in seed_lower_bound..seed_upper_bound {
        let mut rng = ChaChaRng::seed_from_u64(i);

        let mut data = ctor();

        let mut test_vector = Vec::new();

        test_vector.resize(rng.gen_range(1..size_upper_bound), 0);

        rng.fill(&mut test_vector[..]);

        let hash = put_data(&mut data, &test_vector[..]).unwrap();

        let mut to = Vec::new();

        read_data(&data, hash, &mut to).unwrap();

        if to != test_vector {
            dbg!(to.len(), test_vector.len());
            panic!("failed at seed {}", i);
        }
    }
}

#[test]
fn data_round_trip_test() {
    let mut sqlite_ds = || SqliteDs::new(":memory:").unwrap();

    internal_test(&mut sqlite_ds, 1 << 20, 0, 8);
    internal_test(&mut sqlite_ds, 1 << 14, 8, 64);
    internal_test(&mut sqlite_ds, 1 << 10, 64, 128);
}

proptest::proptest! {
    #[test]
    fn identity_read_write(value: Vec<u8>) {
        let mut ds = SqliteDs::new(":memory:").unwrap();

        let key = put_data(&mut ds, &value[..]).unwrap();

        let mut to = Vec::new();

        read_data(&ds, key, &mut to).unwrap();

        assert_eq!(value, to);
    }

    #[test]
    fn between_test(mut keys: HashSet<Vec<u8>>, start: Vec<u8>, end: Option<Vec<u8>>) {
        let ds = SqliteDs::new(":memory:").unwrap();

        keys.retain(|x| !x.is_empty());

        for key in &keys {
            ds.raw_put(key, key).expect("failed to put key");
        }

        let expected_keys: HashSet<Vec<u8>> = if let Some(e) = &end {
            keys.iter().filter(|x| (&start..&e).contains(x)).cloned().collect()
        } else {
            keys.iter().filter(|x| (&start..).contains(x)).cloned().collect()
        };

        let got_keys = ds.raw_between(&start, end.as_deref()).expect("failed to get keys between").into_iter().collect();

        assert_eq!(expected_keys, got_keys);
    }

    // for all valid keys in a data store containing only one item, any length prefix from 2 to the
    // full key must be able to be canonicalized back into the original key
    #[test]
    fn keyish_truncation(value: u64) {
        let sqlite_ds = SqliteDs::new(":memory:").unwrap();
        let blob = value.to_ne_bytes().to_vec();
        let key = sqlite_ds.put(blob).unwrap();

        let keystr = key.to_string();

        for chopped in 2..keystr.len() {
            let s = &keystr[..chopped];

            let keyish: snapcd::keyish::Keyish = s.parse().unwrap();
            assert_eq!(sqlite_ds.canonicalize(keyish).unwrap(), key);
        }
    }

    // TODO: write test for multiple keys existing
    // there must be *some* valid prefix (and once it exists, all suffixes afterwards must be
    // correct and find just that one key)
}

#[test]
fn file_round_trip_test() {
    let dir = tempfile::tempdir().unwrap();

    let input_file_name = dir.path().join("input.bin");
    let mut input_file = std::fs::File::create(&input_file_name).unwrap();

    let mut v = Vec::new();
    v.resize_with(fastrand::usize(0..1_000_000), || fastrand::u8(..));
    input_file.write_all(&v).unwrap();

    let mut sqlite_ds = SqliteDs::new(":memory:").unwrap();

    let hash = snapcd::dir::put_fs_item(&mut sqlite_ds, &input_file_name, &|_| true).unwrap();

    snapcd::dir::get_fs_item(&sqlite_ds, hash, &dir.path().join("output.bin")).unwrap();

    let mut result = Vec::new();
    let mut of = std::fs::File::open(&dir.path().join("output.bin")).unwrap();
    of.read_to_end(&mut result).unwrap();

    assert_eq!(&result, &v);
}
