extern crate quick_patricia_trie;
#[macro_use]
extern crate criterion;
extern crate ethereum_types;
extern crate hashdb;
extern crate keccak_hash;
extern crate keccak_hasher;
extern crate memorydb;
extern crate parity_bytes;
extern crate patricia_trie as trie;
extern crate patricia_trie_ethereum as ethtrie;
extern crate trie_standardmap;

use criterion::{Bencher, Criterion, Fun};

use parity_bytes::Bytes;
//use ethereum_types::H256;
use ethtrie::{TrieDB, TrieDBMut};
use keccak_hash::{keccak, H256};
use keccak_hasher::KeccakHasher;
use memorydb::MemoryDB;
use trie::{Trie, TrieMut};
use trie_standardmap::{Alphabet, StandardMap, ValueMode};

use quick_patricia_trie::trie::Trie as QuickTrie;

fn random_word(alphabet: &[u8], min_count: usize, diff_count: usize, seed: &mut H256) -> Vec<u8> {
    assert!(min_count + diff_count <= 32);
    *seed = keccak(&seed);
    let r = min_count + (seed[31] as usize % (diff_count + 1));
    let mut ret: Vec<u8> = Vec::with_capacity(r);
    for i in 0..r {
        ret.push(alphabet[seed[i] as usize % alphabet.len()]);
    }
    ret
}

fn random_bytes(min_count: usize, diff_count: usize, seed: &mut H256) -> Vec<u8> {
    assert!(min_count + diff_count <= 32);
    *seed = keccak(&seed);
    let r = min_count + (seed[31] as usize % (diff_count + 1));
    seed[0..r].to_vec()
}

fn random_value(seed: &mut H256) -> Bytes {
    *seed = keccak(&seed);
    match seed[0] % 2 {
        1 => vec![seed[31]; 1],
        _ => seed.to_vec(),
    }
}

fn trie_insertion_32_mir_1k(c: &mut Criterion) {
    let st = StandardMap {
        alphabet: Alphabet::All,
        min_key: 32,
        journal_key: 0,
        value_mode: ValueMode::Mirror,
        count: 1000,
    };
    let d = st.make();

    let parity = Fun::new("parity", |b: &mut Bencher, d: &Vec<(Bytes, Bytes)>| {
        b.iter(|| {
            let mut memdb = MemoryDB::<KeccakHasher>::new();
            let mut root = H256::new();
            let mut t = TrieDBMut::new(&mut memdb, &mut root);
            for i in d.iter() {
                t.insert(&i.0, &i.1).unwrap();
            }
        })
    });

    let quick = Fun::new("quick", |b: &mut Bencher, d: &Vec<(Bytes, Bytes)>| {
        b.iter(|| {
            let mut t = QuickTrie::new();
            for i in d.iter() {
                t.insert(&i.0, &i.1);
            }
        })
    });

    let functions = vec![parity, quick];
    c.bench_functions("insertion_32_mir_1k", functions, d);
}

fn trie_insertion_32_ran_1k(c: &mut Criterion) {
    let st = StandardMap {
        alphabet: Alphabet::All,
        min_key: 32,
        journal_key: 0,
        value_mode: ValueMode::Random,
        count: 1000,
    };
    let d = st.make();

    let parity = Fun::new("parity", |b: &mut Bencher, d: &Vec<(Bytes, Bytes)>| {
        b.iter(|| {
            let mut memdb = MemoryDB::<KeccakHasher>::new();
            let mut root = H256::new();
            let mut t = TrieDBMut::new(&mut memdb, &mut root);
            for i in d.iter() {
                t.insert(&i.0, &i.1).unwrap();
            }
        })
    });

    let quick = Fun::new("quick", |b: &mut Bencher, d: &Vec<(Bytes, Bytes)>| {
        b.iter(|| {
            let mut t = QuickTrie::new();
            for i in d.iter() {
                t.insert(&i.0, &i.1);
            }
        })
    });

    let functions = vec![parity, quick];
    c.bench_functions("insertion_32_ran_1k", functions, d);
}

fn trie_insertion_six_high(c: &mut Criterion) {
    let mut d: Vec<(Bytes, Bytes)> = Vec::new();
    let mut seed = H256::new();
    for _ in 0..1000 {
        let k = random_bytes(6, 0, &mut seed);
        let v = random_value(&mut seed);
        d.push((k, v))
    }

    let parity = Fun::new("parity", |b: &mut Bencher, d: &Vec<(Bytes, Bytes)>| {
        b.iter(|| {
            let mut memdb = MemoryDB::<KeccakHasher>::new();
            let mut root = H256::new();
            let mut t = TrieDBMut::new(&mut memdb, &mut root);
            for i in d.iter() {
                t.insert(&i.0, &i.1).unwrap();
            }
        })
    });

    let quick = Fun::new("quick", |b: &mut Bencher, d: &Vec<(Bytes, Bytes)>| {
        b.iter(|| {
            let mut t = QuickTrie::new();
            for i in d.iter() {
                t.insert(&i.0, &i.1);
            }
        })
    });

    let functions = vec![parity, quick];
    c.bench_functions("insertion_six_high", functions, d);
}

fn trie_insertion_six_mid(c: &mut Criterion) {
    let alphabet = b"@QWERTYUIOPASDFGHJKLZXCVBNM[/]^_";
    let mut d: Vec<(Bytes, Bytes)> = Vec::new();
    let mut seed = H256::new();
    for _ in 0..1000 {
        let k = random_word(alphabet, 6, 0, &mut seed);
        let v = random_value(&mut seed);
        d.push((k, v))
    }

    let parity = Fun::new("parity", |b: &mut Bencher, d: &Vec<(Bytes, Bytes)>| {
        b.iter(|| {
            let mut memdb = MemoryDB::<KeccakHasher>::new();
            let mut root = H256::new();
            let mut t = TrieDBMut::new(&mut memdb, &mut root);
            for i in d.iter() {
                t.insert(&i.0, &i.1).unwrap();
            }
        })
    });

    let quick = Fun::new("quick", |b: &mut Bencher, d: &Vec<(Bytes, Bytes)>| {
        b.iter(|| {
            let mut t = QuickTrie::new();
            for i in d.iter() {
                t.insert(&i.0, &i.1);
            }
        })
    });

    let functions = vec![parity, quick];
    c.bench_functions("insertion_six_mid", functions, d);
}

fn trie_insertion_random_mid(c: &mut Criterion) {
    let alphabet = b"@QWERTYUIOPASDFGHJKLZXCVBNM[/]^_";
    let mut d: Vec<(Bytes, Bytes)> = Vec::new();
    let mut seed = H256::new();
    for _ in 0..1000 {
        let k = random_word(alphabet, 1, 5, &mut seed);
        let v = random_value(&mut seed);
        d.push((k, v))
    }

    let parity = Fun::new("parity", |b: &mut Bencher, d: &Vec<(Bytes, Bytes)>| {
        b.iter(|| {
            let mut memdb = MemoryDB::<KeccakHasher>::new();
            let mut root = H256::new();
            let mut t = TrieDBMut::new(&mut memdb, &mut root);
            for i in d.iter() {
                t.insert(&i.0, &i.1).unwrap();
            }
        })
    });

    let quick = Fun::new("quick", |b: &mut Bencher, d: &Vec<(Bytes, Bytes)>| {
        b.iter(|| {
            let mut t = QuickTrie::new();
            for i in d.iter() {
                t.insert(&i.0, &i.1);
            }
        })
    });

    let functions = vec![parity, quick];
    c.bench_functions("insertion_random_mid", functions, d);
}

fn trie_iter(c: &mut Criterion) {
    let st = StandardMap {
        alphabet: Alphabet::All,
        min_key: 32,
        journal_key: 0,
        value_mode: ValueMode::Mirror,
        count: 1000,
    };
    let d = st.make();

    let mut memdb = MemoryDB::<KeccakHasher>::new();
    let mut root = H256::new();
    {
        let mut t = TrieDBMut::new(&mut memdb, &mut root);
        for i in d.iter() {
            t.insert(&i.0, &i.1).unwrap();
        }
    }
    let parity = Fun::new("parity", move |b: &mut Bencher, _d: &()| {
        b.iter(|| {
            let t = TrieDB::new(&memdb, &root).unwrap();
            for n in t.iter().unwrap() {
                let _ = n;
            }
        })
    });

    let mut t = QuickTrie::new();
    for i in d.iter() {
        t.insert(&i.0, &i.1);
    }
    let quick = Fun::new("quick", move |b: &mut Bencher, _d: &()| {
        b.iter(|| {
            for n in t.iter() {
                let _ = n;
            }
        })
    });

    let functions = vec![parity, quick];
    c.bench_functions("iter", functions, ());
}

//criterion_group!(benches, trie_insertion_32_mir_1k);
criterion_group!(
    benches,
    trie_insertion_32_mir_1k,
    trie_insertion_32_ran_1k,
    trie_insertion_six_high,
    trie_insertion_six_mid,
    trie_insertion_random_mid,
    trie_iter
);
criterion_main!(benches);
