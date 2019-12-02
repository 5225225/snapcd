use std::borrow::Cow;
use std::io::prelude::*;
use blake2::{Blake2b, Digest};
use std::collections::HashMap;
use rand::prelude::*;
use rand::RngCore;
use rand_chacha::ChaChaRng;
use std::mem;
use cdc::RollingHash64;
use std::io::BufReader;
use rusqlite::{params, Connection};
use hex::ToHex;

#[derive(Debug, Clone, Copy)]
pub struct Key<'a>(&'a [u8]);

#[derive(Debug, Default, Clone)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct KeyBuf(Vec<u8>);

impl KeyBuf {
    pub fn as_key<'a>(&'a self) -> Key<'a> {
        Key(&self.0[..])
    }
}

impl std::fmt::Display for KeyBuf {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        fmt.write_str(&hex::encode(&self.0))
    }
}

impl std::str::FromStr for KeyBuf {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(KeyBuf(hex::decode(s).unwrap()))
    }
}

#[derive(Debug)]
#[derive(serde::Serialize, serde::Deserialize)]
pub enum Object<'a> {
    Blob(Cow<'a, [u8]>),
    Keys(Cow<'a, [KeyBuf]>),
}

pub trait DataStore {
    fn get<'a>(&'a self, key: Key) -> Cow<'a, [u8]>;
    fn put(&mut self, data: Vec<u8>) -> KeyBuf;

    fn get_obj(&self, key: Key) -> Object {
        let data = self.get(key);

        serde_cbor::from_slice(&data).unwrap()
    }

    fn put_obj(&mut self, data: &Object) -> KeyBuf {
        let data = serde_cbor::to_vec(data).unwrap();

        self.put(data)
    }

    fn put_data<R: Read>(&mut self, data: R) -> KeyBuf {
        let mut key_bufs: [Vec<KeyBuf>; 5] = Default::default();

        let mut current_chunk = Vec::new();

        let mut hasher = cdc::Rabin64::new(6);

        for byte_r in data.bytes() {
            let byte = byte_r.unwrap();

            current_chunk.push(byte);
            hasher.slide(&byte);

            let h = !hasher.get_hash();

            let zeros = h.trailing_zeros();

            if zeros > BLOB_ZERO_COUNT || current_chunk.len() >= 1<<(BLOB_ZERO_COUNT+1) {
                hasher.reset();

                let key = self.put_obj(&Object::Blob(Cow::Borrowed(&current_chunk)));
                key_bufs[0].push(key);
                current_chunk.clear();

                for offset in 0..4 {
                    if zeros > BLOB_ZERO_COUNT + (offset + 1) * PER_LEVEL_COUNT || key_bufs[offset as usize].len() >= 1<<(PER_LEVEL_COUNT+1) { 
                        let key = self.put_obj(&Object::Keys(Cow::Borrowed(&key_bufs[offset as usize])));
                        key_bufs[offset as usize].clear();
                        key_bufs[offset as usize + 1].push(key);
                    } else {
                        continue;
                    }
                }
            }

        }

        println!("#{} {:?}", current_chunk.len(), &key_bufs.iter().map(|x| x.len()).collect::<Vec<_>>());
        if current_chunk.len() > 0 {
            let data = mem::replace(&mut current_chunk, Vec::new());
            let key = self.put_obj(&Object::Blob(Cow::Borrowed(&data)));
            key_bufs[0].push(key);
        }

        for offset in 0..4 {
            println!("!{} {} {:?}", offset, current_chunk.len(), &key_bufs.iter().map(|x| x.len()).collect::<Vec<_>>());
            let keys = mem::replace(&mut key_bufs[offset], Vec::new());
            let key = self.put_obj(&Object::Keys(Cow::Borrowed(&keys)));
            key_bufs[offset + 1].push(key);
        }
        println!("^{} {:?}", current_chunk.len(), &key_bufs.iter().map(|x| x.len()).collect::<Vec<_>>());

        assert!(key_bufs[0].len() == 0);
        assert!(key_bufs[1].len() == 0);
        assert!(key_bufs[2].len() == 0);
        assert!(key_bufs[3].len() == 0);

        let taken = mem::replace(&mut key_bufs[4], Vec::new());
        return self.put_obj(&Object::Keys(Cow::Borrowed(&taken)));
    }

    fn read_data<W: Write>(&self, key: Key, to: &mut W) {
        let obj = self.get_obj(key);

        match obj { 
            Object::Keys(keys) => {
                for key in keys.iter() {
                    self.read_data(key.as_key(), to);
                }
            }
            Object::Blob(vec) => {
                to.write(&vec).expect("failed to write to out");
            }
        }
    }
}

pub struct SqliteDS {
    conn: rusqlite::Connection,
}

impl SqliteDS {
    pub fn new(path: &str) -> Self {
        let conn = rusqlite::Connection::open(path).unwrap();

        conn.pragma_update(None, &"journal_mode", &"WAL").unwrap();

        conn.execute("
            CREATE TABLE IF NOT EXISTS data (
                key BLOB NOT NULL UNIQUE PRIMARY KEY,
                value BLOB NOT NULL
            ) WITHOUT ROWID
        ", params![]).unwrap();

        Self {
            conn
        }
    }
}

impl DataStore for SqliteDS {
    fn get<'a>(&'a self, key: Key) -> Cow<'a, [u8]> {
        let results: Vec<u8> = self.conn.query_row(
            "SELECT value FROM data WHERE key=?",
            params![key.0],
            |row| row.get(0)).unwrap();

        Cow::Owned(results)
    }

    fn put(&mut self, data: Vec<u8>) -> KeyBuf {
        let mut b2 = Blake2b::new();
        b2.input(&data);
        let hash = b2.result().to_vec();

        self.conn.execute(
            "INSERT INTO data VALUES (?, ?)",
            params![hash, data]);

        KeyBuf(hash)
    }
}

const BLOB_ZERO_COUNT: u32 = 11;
const PER_LEVEL_COUNT: u32 = 5;

pub fn put_data<DS: DataStore, R: Read>(data: R, store: &mut DS) -> KeyBuf {
    store.put_data(data)
}

pub fn read_data<DS: DataStore, W: Write>(key: Key, store: &DS, to: &mut W) {
    store.read_data(key, to);
}

#[derive(Debug, Default)]
pub struct HashSetDS {
    data: HashMap<Vec<u8>, Vec<u8>>,
}

impl DataStore for HashSetDS {
    fn get<'a>(&'a self, key: Key) -> Cow<'a, [u8]> {
        Cow::Borrowed(&self.data[&*key.0])
    }

    fn put(&mut self, data: Vec<u8>) -> KeyBuf {
        let mut b2 = Blake2b::new();
        b2.input(&data);
        let hash = b2.result().to_vec();
        self.data.insert(hash.clone(), data);
        KeyBuf(hash)
    }
}

#[derive(Debug, Default)]
pub struct NullB2DS {
}

impl DataStore for NullB2DS {
    fn get<'a>(&'a self, key: Key) -> Cow<'a, [u8]> {
        Cow::Borrowed(&[0; 0])
    }

    fn put(&mut self, data: Vec<u8>) -> KeyBuf {
        let mut b2 = Blake2b::new();
        b2.input(&data);
        let hash = b2.result().to_vec();
        KeyBuf(hash)
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
    for i in 0..256 {

        let mut rng = ChaChaRng::seed_from_u64(i);

        let mut data = HashSetDS::default();

        let mut test_vector = Vec::new();

        test_vector.resize(rng.gen_range(1, 1<<20), 0);

        rng.fill(&mut test_vector[..]);

        let hash = put_data(&test_vector[..], &mut data);

        let mut to = Vec::new();

        read_data(hash.as_key(), &data, &mut to);

        if to != test_vector {
            panic!("failed at seed {}", i);
        }
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
    read_data(hash.as_key(), &data, &mut to);
    assert_eq!(to, test_vector);
    print_stats(&data);

    let dist = rand::distributions::Bernoulli::new(1_f64/100000_f64).unwrap();

    test_vector.retain(|_| !dist.sample(&mut rng));

    let hash = put_data(&test_vector[..], &mut data);
    let mut to = Vec::new();
    read_data(hash.as_key(), &data, &mut to);
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

    let bar = rng.take(1<<20);

    println!("{:?}", put_data(bar, &mut data));
}
