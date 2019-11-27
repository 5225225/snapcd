use std::io::prelude::*;
use blake2::{Blake2b, Digest};
use std::collections::HashMap;
use rand::prelude::*;
use rand::RngCore;
use rand_chacha::ChaChaRng;
use std::mem;
use cdc::RollingHash64;

#[derive(Debug, Default)]
#[derive(serde::Serialize, serde::Deserialize)]
struct Key(Vec<u8>);

#[derive(Debug)]
#[derive(serde::Serialize, serde::Deserialize)]
enum Object {
    Blob(Vec<u8>),
    Keys(Vec<Key>),
}

trait DataStore {
    fn get(&self, key: &Key) -> &[u8];
    fn put(&mut self, data: Vec<u8>) -> Key;

    fn get_obj(&self, key: &Key) -> Object {
        let data = self.get(key);

        serde_cbor::from_slice(data).unwrap()
    }

    fn put_obj(&mut self, data: &Object) -> Key {
        let data = serde_cbor::to_vec(data).unwrap();

        self.put(data)
    }
}

fn put_data<DS: DataStore, R: Read>(data: R, store: &mut DS) -> Key {
    let mut key_bufs: [Vec<Key>; 5] = Default::default();

    let mut current_chunk = Vec::with_capacity(4096);

    let mut hasher = cdc::Rabin64::new(6);
    let h2l = cdc::HashToLevel::custom_new(12, 4);

    for byte_r in data.bytes() {
        let byte = byte_r.unwrap();
        hasher.slide(&byte);
        current_chunk.push(byte);

        let h = hasher.get_hash();

        let level = h2l.to_level(*h);

        if level > 0 {
            let data = mem::replace(&mut current_chunk, Vec::with_capacity(4096));
            let key = store.put_obj(&Object::Blob(data));
            key_bufs[0].push(key);

            for offset in 0..4 {
                if level > offset+1 { 
                    let keys = mem::replace(&mut key_bufs[offset], Vec::new());
                    let key = store.put_obj(&Object::Keys(keys));
                    key_bufs[offset + 1].push(key);
                } else {
                    continue;
                }
            }
        }

    }

    println!("#{} {:?}", current_chunk.len(), &key_bufs.iter().map(|x| x.len()).collect::<Vec<_>>());
    if current_chunk.len() > 0 {
        let data = mem::replace(&mut current_chunk, Vec::new());
        let key = store.put_obj(&Object::Blob(data));
        key_bufs[0].push(key);
    }

    for offset in 0..4 {
        println!("!{} {} {:?}", offset, current_chunk.len(), &key_bufs.iter().map(|x| x.len()).collect::<Vec<_>>());
        let keys = mem::replace(&mut key_bufs[offset], Vec::new());
        let key = store.put_obj(&Object::Keys(keys));
        key_bufs[offset + 1].push(key);
    }
    println!("^{} {:?}", current_chunk.len(), &key_bufs.iter().map(|x| x.len()).collect::<Vec<_>>());

    assert!(key_bufs[0].len() == 0);
    assert!(key_bufs[1].len() == 0);
    assert!(key_bufs[2].len() == 0);
    assert!(key_bufs[3].len() == 0);

    let taken = mem::replace(&mut key_bufs[4], Vec::new());
    return store.put_obj(&Object::Keys(taken));
}

fn read_data<DS: DataStore, W: Write>(key: &Key, store: &DS, to: &mut W) {
    let obj = store.get_obj(key);

    //dbg!(&obj);

    match obj { 
        Object::Keys(keys) => {
            for key in keys {
                read_data(&key, store, to);
            }
        }
        Object::Blob(vec) => {
            to.write(&vec).expect("failed to write to out");
        }
    }


}

#[derive(Debug, Default)]
struct HashSetDS {
    data: HashMap<Vec<u8>, Vec<u8>>,
}

impl DataStore for HashSetDS {
    fn get(&self, key: &Key) -> &[u8] {
        &self.data[&key.0]
    }

    fn put(&mut self, data: Vec<u8>) -> Key {
        let mut b2 = Blake2b::new();
        b2.input(&data);
        let hash = b2.result().to_vec();
        self.data.insert(hash.clone(), data);
        Key(hash)
    }
}

#[derive(Debug, Default)]
struct NullB2DS {
}

impl DataStore for NullB2DS {
    fn get(&self, _key: &Key) -> &[u8] {
        &[0; 0]
    }

    fn put(&mut self, data: Vec<u8>) -> Key {
        let mut b2 = Blake2b::new();
        b2.input(&data);
        let hash = b2.result().to_vec();
        Key(hash)
    }
}

fn print_stats(data: &HashSetDS) {
    println!("keys: {}", data.data.len());
    println!("total values len: {}", data.data.iter().map(|x| x.1.len()).sum::<usize>());
    println!("total K/V len: {}", data.data.iter().map(|x| x.0.len() + x.1.len()).sum::<usize>());
    println!("min val len: {:?}", data.data.iter().map(|x| x.1.len()).min());
    println!("max val len: {:?}", data.data.iter().map(|x| x.1.len()).max());
}

fn sanity_check() {
    for i in 0..(1<<32) {

        let mut rng = ChaChaRng::seed_from_u64(i);

        let mut data = HashSetDS::default();

        let mut test_vector = Vec::new();

        test_vector.resize(rng.gen_range(1, 1<<24), 0);

        rng.fill(&mut test_vector[..]);

        let hash = put_data(&test_vector[..], &mut data);

        let mut to = Vec::new();

        read_data(&hash, &data, &mut to);

        /*
        for key in &data.data {
            println!("{:?}\n", key);
        }

        println!("\n{:?}", &hash);

        // */

        println!("testing seed {}", i);
        assert!(to == test_vector);

//        print_stats(&data);
    }
}

fn size_check() {
    let mut data = HashSetDS::default();

    let mut rng = ChaChaRng::seed_from_u64(0);

    let mut test_vector = Vec::new();

    test_vector.resize(rng.gen_range(1<<25, 1<<26), 0);

    rng.fill(&mut test_vector[..]);
    let hash = put_data(&test_vector[..], &mut data);
    let mut to = Vec::new();
    read_data(&hash, &data, &mut to);
    assert_eq!(to, test_vector);
    print_stats(&data);

    let dist = rand::distributions::Bernoulli::new(1_f64/100000_f64).unwrap();

    test_vector.retain(|_| !dist.sample(&mut rng));

    let hash = put_data(&test_vector[..], &mut data);
    let mut to = Vec::new();
    read_data(&hash, &data, &mut to);
    assert_eq!(to, test_vector);
    print_stats(&data);

    put_data(&test_vector[..1<<16], &mut data);
    put_data(&test_vector[..1<<17], &mut data);
    put_data(&test_vector[..1<<18], &mut data);

    print_stats(&data);
}

fn test_infinite() {
    let mut data = NullB2DS::default();

    for i in 0..64 {
        let rng: Box<dyn RngCore> = Box::new(ChaChaRng::seed_from_u64(1));

        let bar = rng.take(1<<i);

        println!("running on {}", i);
        put_data(bar, &mut data);
    }
}

fn perf_test() {
    let mut data = NullB2DS::default();

    let rng: Box<dyn RngCore> = Box::new(ChaChaRng::seed_from_u64(1));

    let bar = rng.take(1<<30);

    println!("{:?}", put_data(bar, &mut data));
}

fn main() {
    sanity_check();
//    size_check();
//    test_infinite();
//    perf_test();
}
