use std::io::prelude::*;
use cdchunking::{Chunker, ZPAQ};
use blake2::{Blake2b, Digest};
use std::collections::{HashMap, VecDeque};
use rand::prelude::*;
use rand::RngCore;
use rand_chacha::ChaChaRng;
use std::mem;

mod fixedadler;

#[derive(Debug, Default)]
#[derive(serde::Serialize, serde::Deserialize)]
struct Key(Vec<u8>);

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

const WINDOW_LENGTH: usize = 32;

fn put_data<DS: DataStore, R: Read>(data: R, store: &mut DS) -> Vec<u8> {
    let mut key_bufs: [Vec<Key>; 16] = Default::default();

    let mut current_chunk = Vec::new();

    let mut hasher = fixedadler::FixedSizeAdler32::new(WINDOW_LENGTH);

    for byte_r in data.bytes() {
        let byte = byte_r.unwrap();
        hasher.write(&[byte]).unwrap();
        current_chunk.push(byte);

        if hasher.hash() % (1<<12) == 0 {
            let data = mem::replace(&mut current_chunk, Vec::new());
            let key = store.put_obj(&Object::Blob(data));
            key_bufs[0].push(key);
        }

        for offset in 0..15 {
            if hasher.hash() % (1<<(12 + offset*4)) == 0 { 
                let keys = mem::replace(&mut key_bufs[offset], Vec::new());
                let key = store.put_obj(&Object::Keys(keys));
                key_bufs[offset + 1].push(key);
            }
        }
    }

    unimplemented!()
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
    fn get(&self, key: &Key) -> &[u8] {
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
}

fn main() {
    let mut data = NullB2DS::default();

    let test_rng: Box<dyn RngCore> = Box::new(ChaChaRng::seed_from_u64(1));

    let hash = put_data(test_rng.take(1<<16), &mut data);
}
