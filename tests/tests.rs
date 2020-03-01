use rand::prelude::*;
use rand_chacha::ChaChaRng;
use snapcd::file::{put_data, read_data};
use snapcd::{ds::sqlite::SqliteDS, DataStore};

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

        test_vector.resize(rng.gen_range(1, size_upper_bound), 0);

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
fn sanity_check() {
    let mut sqlite_ds = || SqliteDS::new(":memory:").unwrap();

    internal_test(&mut sqlite_ds, 1 << 20, 0, 8);
    internal_test(&mut sqlite_ds, 1 << 14, 8, 64);
    internal_test(&mut sqlite_ds, 1 << 10, 64, 128);
}

proptest::proptest! {
    #[test]
    fn identity_read_write(value: Vec<u8>) {
        let mut ds = SqliteDS::new(":memory:").unwrap();

        let key = put_data(&mut ds, &value[..]).unwrap();

        let mut to = Vec::new();

        read_data(&ds, key, &mut to).unwrap();

        assert_eq!(value, to);
    }

    #[test]
    fn between_test(keys: Vec<Vec<u8>>, start: Vec<u8>, end: Option<Vec<u8>>) {
        let ds = SqliteDS::new(":memory:").unwrap();

        for key in &keys {
            ds.raw_put(key, key).expect("failed to put key");
        }

        let mut expected_keys: Vec<Vec<u8>> = if let Some(e) = &end {
            keys.iter().filter(|x| (&start..&e).contains(x)).cloned().collect()
        } else {
            keys.iter().filter(|x| (&start..).contains(x)).cloned().collect()
        };

        let mut got_keys = ds.raw_between(&start, end.as_deref()).expect("failed to get keys between");

        expected_keys.sort();
        got_keys.sort();

        assert_eq!(expected_keys, got_keys);
    }
}
