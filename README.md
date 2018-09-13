# quick-patricia-tree

A toy implementation of a patricia tree as [used in ethereum][1].

The implementation is strongly inspired by [parity patricia-trie][2].

## Key difference

This is a total reimplementation from scratch so there are numerous differences. 

The major ones are:

* much less generic: everything is a `&[u8]` at some point. The only requirements is for
the key/value being `AsRef[u8]>`.
* use of [`Arena`][3], a struct in charge of allocating all data. As a result, there is 
no need to use *elastic-array* or some other fixed array based crates. Everything is
eventually stored in a unique `Vec`, all references are instead indexes that we can freely
copy and lookups are thus very fast.
* there is no backend database yet. While it will probably be done in the future, I wanted
to implement the core idea of the Merkle Patricia Trie as defined in the [ethereum doc][1]
and benchmark it against the parity one based on memory-db.
* it is probably lacking many more features I are so far unecessary (like the iterator seek)

# Benchmarks

So far results are promising. There is a [criterion crate][4] which tries to run the same
benches as written in the original parity crates.

## Trie insertion mirror 1k

![trie_insertion_mir_1k][5]

## Trie insertion random 1k

![trie_insertion_mir_1k][6]

## Trie insertion six high

![trie_insertion_six_high][7]

## Trie insertion six mid

![trie_insertion_six_mid][8]

## Trie insertion random mid

![trie_insertion_random_mid][9]

## Iterator

![iter][10]

[1]: https://github.com/ethereum/wiki/wiki/Patricia-Tree
[2]: https://crates.io/crates/patricia-trie
[3]: /src/arena.rs
[4]: /criterion_benches
[5]: /criterion_benches/target/criterion/insertion_32_mir_1k/report/violin.svg
[6]: /criterion_benches/target/criterion/insertion_32_ran_1k/report/violin.svg
[7]: /criterion_benches/target/criterion/insertion_six_high/report/violin.svg
[8]: /criterion_benches/target/criterion/insertion_six_mid/report/violin.svg
[9]: /criterion_benches/target/criterion/insertion_random_mid/report/violin.svg
[10]: /criterion_benches/target/criterion/iter/report/violin.svg
