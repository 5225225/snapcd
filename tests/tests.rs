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
