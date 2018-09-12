extern crate quick_patricia_trie;
#[macro_use]
extern crate criterion;
extern crate parity_bytes;
extern crate ethereum_types;
extern crate memorydb;
extern crate patricia_trie as trie;
extern crate patricia_trie_ethereum as ethtrie;
extern crate keccak_hasher;
extern crate keccak_hash;
extern crate trie_standardmap;
extern crate hashdb;

use criterion::{Criterion, Bencher, Fun};

use parity_bytes::Bytes;
//use ethereum_types::H256;
use keccak_hash::{H256, keccak};
use memorydb::MemoryDB;
use trie::{TrieMut, Trie};
use trie_standardmap::{Alphabet, ValueMode, StandardMap};
use keccak_hasher::KeccakHasher;
use ethtrie::{TrieDB, TrieDBMut};

use quick_patricia_trie::trie::Trie as QuickTrie;

// fn random_word(alphabet: &[u8], min_count: usize, diff_count: usize, seed: &mut H256) -> Vec<u8> {
//     assert!(min_count + diff_count <= 32);
//     *seed = keccak(&seed);
//     let r = min_count + (seed[31] as usize % (diff_count + 1));
//     let mut ret: Vec<u8> = Vec::with_capacity(r);
//     for i in 0..r {
//         ret.push(alphabet[seed[i] as usize % alphabet.len()]);
//     }
//     ret
// }
// 
// fn random_bytes(min_count: usize, diff_count: usize, seed: &mut H256) -> Vec<u8> {
//     assert!(min_count + diff_count <= 32);
//     *seed = keccak(&seed);
//     let r = min_count + (seed[31] as usize % (diff_count + 1));
//     seed[0..r].to_vec()
// }
// 
// fn random_value(seed: &mut H256) -> Bytes {
//     *seed = keccak(&seed);                                   
//     match seed[0] % 2 {                                                     
//         1 => vec![seed[31];1],
//         _ => seed.to_vec(),
//     }
// }

fn trie_insertion_32_mir_1k(c: &mut Criterion) {
    let st = StandardMap {
        alphabet: Alphabet::All,
        min_key: 32,
        journal_key: 0,
        value_mode: ValueMode::Mirror,
        count: 1000,
    };
    let d = st.make();

    let parity = Fun::new("parity", |b: &mut Bencher, d: &Vec<(Bytes, Bytes)>| b.iter(|| {
        let mut memdb = MemoryDB::<KeccakHasher>::new();
        let mut root = H256::new();
        let mut t = TrieDBMut::new(&mut memdb, &mut root);
        for i in d.iter() {
            t.insert(&i.0, &i.1).unwrap();
        }
    }));

    let quick = Fun::new("quick", |b: &mut Bencher, d: &Vec<(Bytes, Bytes)>| b.iter(|| {
        let mut t = QuickTrie::new();
        for i in d.iter() {
            t.insert(&i.0, &i.1);
        }
    }));

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

    let parity = Fun::new("parity", |b: &mut Bencher, d: &Vec<(Bytes, Bytes)>| b.iter(|| {
        let mut memdb = MemoryDB::<KeccakHasher>::new();
        let mut root = H256::new();
        let mut t = TrieDBMut::new(&mut memdb, &mut root);
        for i in d.iter() {
            t.insert(&i.0, &i.1).unwrap();
        }
    }));

    let quick = Fun::new("quick", |b: &mut Bencher, d: &Vec<(Bytes, Bytes)>| b.iter(|| {
        let mut t = QuickTrie::new();
        for i in d.iter() {
            t.insert(&i.0, &i.1);
        }
    }));

    let functions = vec![parity, quick];
    c.bench_functions("insertion_32_ran_1k", functions, d);
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
    let parity = Fun::new("parity", move |b: &mut Bencher, _d: &()| b.iter(|| {
        let t = TrieDB::new(&memdb, &root).unwrap();
        for n in t.iter().unwrap() {
            let _ = n;
        }
    }));

    let mut t = QuickTrie::new();
    for i in d.iter() {
        t.insert(&i.0, &i.1);
    }
    let quick = Fun::new("quick", move |b: &mut Bencher, _d: &()| b.iter(|| {
        for n in t.iter() {
            let _ = n;
        }
    }));

    let functions = vec![parity, quick];
    c.bench_functions("iter", functions, ());
}


//criterion_group!(benches, trie_insertion_32_mir_1k);
criterion_group!(benches, trie_insertion_32_mir_1k, trie_insertion_32_ran_1k, trie_iter);
criterion_main!(benches);
