# Another experiment in Bitcoin indexing

I want to clean up bunch of code from [rust-bitcoin-indexer](https://github.com/dpc/rust-bitcoin-indexer),
and then model it after [blocks_iterator](https://github.com/RCasatta/blocks_iterator), and make it modular
and easy to bench, eventually decided to copy a lot code from `blocks_iterator` altogether.

So that's what it's at right now.
