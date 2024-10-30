# Palomadex Core

Multi pool type automated market-maker (AMM) protocol powered by smart contracts on the Palomachain.

## General Contracts

| Name                                               | Description                                                         |
|----------------------------------------------------|---------------------------------------------------------------------|
| [`factory`](contracts/factory)                     | Pool creation factory                                               |
| [`pair`](contracts/pair)                           | Pair with x*y=k curve                                               |
| [`pair_stable`](contracts/pair_stable)             | Pair with stableswap invariant curve                                |
| [`router`](contracts/router)                       | Multi-hop trade router                                              |

### You can compile each contract:

Go to contract directory and run

```
cargo wasm
cp ../../target/wasm32-unknown-unknown/release/palomadex_token.wasm .
ls -l palomadex_token.wasm
sha256sum palomadex_token.wasm
```

### You can run tests for all contracts

Run the following from the repository root

```
cargo test
```

### For a production-ready (compressed) build:

Run the following from the repository root

```
./scripts/build_release.sh
```

The optimized contracts are generated in the artifacts/ directory.

