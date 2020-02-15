use rand::prelude::*;
use rand_chacha::ChaChaRng;
use snapcd::file::{put_data, read_data};
use snapcd::{ds::sled::SledDS, DataStore, SqliteDS};

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

        read_data(&data, &hash, &mut to).unwrap();

        if to != test_vector {
            dbg!(to.len(), test_vector.len());
            panic!("failed at seed {}", i);
        }
    }
}

#[test]
fn sanity_check() {
    let mut sqliteDS = || SqliteDS::new(":memory:").unwrap();
    let mut sledDS = || SledDS::new_tmp().unwrap();

    internal_test(&mut sqliteDS, 1 << 16, 0, 8);
    internal_test(&mut sqliteDS, 1 << 10, 8, 64);
    internal_test(&mut sqliteDS, 1 << 6, 64, 128);

    internal_test(&mut sledDS, 1 << 16, 0, 8);
    internal_test(&mut sledDS, 1 << 10, 8, 64);
    internal_test(&mut sledDS, 1 << 6, 64, 128);
}

use snapcd::{KeyBuf, Keyish};
use std::str::FromStr;

proptest::proptest! {
    #[test]
    fn round_trip_blake3b_db(bytes: [u8; 32]) {
        let k = KeyBuf::Blake3B(bytes);
        let as_db = k.as_db_key();
        let from_db = KeyBuf::from_db_key(&as_db);
        assert_eq!(k, from_db);
    }

    #[test]
    fn round_trip_base32(bytes: Vec<u8>) {
        let b32 = snapcd::to_base32(&bytes);
        let restored = snapcd::from_base32(&b32, bytes.len() * 8).unwrap();
        assert_eq!(restored.as_slice(), &*bytes);
    }

    #[test]
    fn round_trip_blake3b_to_keyish(bytes: [u8; 32]) {
        let k = KeyBuf::Blake3B(bytes);
        let as_user = k.as_user_key();
        let from_user = Keyish::from_str(&as_user).unwrap();


        if let Keyish::Key(_, b) = from_user {
            assert_eq!(b, k.as_db_key(), "keyish from_str did not round trip with as_user_key");

            let db_key = KeyBuf::from_db_key(&b);
            if let KeyBuf::Blake3B(newbytes) = db_key {
                assert_eq!(bytes, newbytes);
            } else {
                panic!("parsed a keybuf that wasn't expected hash type")
            }
        } else {
            panic!("we asked for the full key and we got something else")
        }
    }
}
