use libsnapcd::file::{put_data, read_data};
use libsnapcd::{ds::sqlite::SqliteDs, ds::DataStore};
use proptest::prelude::*;
use rand::prelude::*;
use rand_chacha::ChaChaRng;
use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::Path;

use cap_std::fs::Dir;

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

proptest! {
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
            keys.iter().filter(|x| (&start..e).contains(x)).cloned().collect()
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

            let keyish: libsnapcd::keyish::Keyish = s.parse().unwrap();
            assert_eq!(sqlite_ds.canonicalize(keyish).unwrap(), key);
        }
    }
}

proptest! {
    // this is a slower test but still good to run
    #![proptest_config(ProptestConfig::with_cases(32))]
    #[test]
    fn keyishes_truncation(mut values: HashSet<u64>) {
        let sqlite_ds = SqliteDs::new(":memory:").unwrap();

        for value in &values {
            let blob = value.to_ne_bytes().to_vec();
            let key = sqlite_ds.put(blob).unwrap();
            let keystr = key.to_string();

            let mut found = false;

            for chopped in 2..keystr.len() {
                let s = &keystr[..chopped];

                let keyish: libsnapcd::keyish::Keyish = s.parse().unwrap();
                // TODO: reintroduce error for this test
                use libsnapcd::ds::CanonicalizeError;

                match sqlite_ds.canonicalize(keyish) {
                    Ok(k) => {
                        assert_eq!(k, key);
                        found = true;
                    },
                    Err(CanonicalizeError::Ambigious(_input, cands)) => {
                        assert!(!found);
                        assert!(cands.contains(&key));
                    },
                    Err(e) => panic!("other error: {}", e),
                }
            }
        }
    }
}

fn create_test_data(dir: &Dir, fname: impl AsRef<Path>) -> Vec<u8> {
    let mut f = dir.create(fname).unwrap();
    let mut v = Vec::new();
    v.resize_with(fastrand::usize(0..1_000_000), || fastrand::u8(..));
    f.write_all(&v).unwrap();
    v
}

#[test]
fn file_put_test() {
    let dir = cap_tempfile::tempdir(cap_std::ambient_authority()).unwrap();

    create_test_data(&dir, "input.bin");

    let input_file = dir.open("input.bin").unwrap();

    let mut sqlite_ds = SqliteDs::new(":memory:").unwrap();

    let input_entry = input_file.into();

    libsnapcd::dir::put_fs_item(&mut sqlite_ds, &input_entry, "".into(), &|_| true).unwrap();
}

#[test]
fn file_round_trip_test() {
    let dir = cap_tempfile::tempdir(cap_std::ambient_authority()).unwrap();

    let test_data_vec = create_test_data(&dir, "input.bin");

    let mut sqlite_ds = SqliteDs::new(":memory:").unwrap();

    let input_entry = dir.open("input.bin").unwrap().into();

    let hash =
        libsnapcd::dir::put_fs_item(&mut sqlite_ds, &input_entry, "".into(), &|_| true).unwrap();

    let output_file = dir.create("output.bin").unwrap();
    libsnapcd::dir::get_fs_item_file(&sqlite_ds, hash, &output_file).unwrap();

    let mut output_file_std = dir.open("output.bin").unwrap();
    let mut result = Vec::new();
    output_file_std.read_to_end(&mut result).unwrap();

    assert_eq!(&result, &test_data_vec);
}

#[test]
fn chunker_works() {
    use libsnapcd::{ds::DataStore, key::Key, object::Object};
    use std::collections::HashSet;

    let sqlite_ds = SqliteDs::new(":memory:").unwrap();

    let mut data = Vec::new();
    data.resize_with(1 << 20, rand::random);
    let cursor = std::io::Cursor::new(&data);

    let mut seen_chunks = HashSet::new();

    libsnapcd::file::inner_put_data(
        cursor,
        sqlite_ds.get_gearhash_table(),
        &mut |data: &[u8]| {
            seen_chunks.insert(data.to_vec());
            sqlite_ds.put_obj(&Object::FileBlob { buf: data.to_vec() })
        },
        &mut |keys: &[Key]| {
            sqlite_ds.put_obj(&Object::FileBlobTree {
                keys: keys.to_vec(),
            })
        },
    )
    .expect("failed to put?");

    let before = seen_chunks.len();

    data[rand::random::<usize>() % (1 << 20)] += 1;

    let cursor = std::io::Cursor::new(&data);
    libsnapcd::file::inner_put_data(
        cursor,
        sqlite_ds.get_gearhash_table(),
        &mut |data: &[u8]| {
            seen_chunks.insert(data.to_vec());
            sqlite_ds.put_obj(&Object::FileBlob { buf: data.to_vec() })
        },
        &mut |keys: &[Key]| {
            sqlite_ds.put_obj(&Object::FileBlobTree {
                keys: keys.to_vec(),
            })
        },
    )
    .expect("failed to put?");

    let after = seen_chunks.len();

    assert!(after - before <= 2);

    dbg!(after);
    dbg!(before);
}
