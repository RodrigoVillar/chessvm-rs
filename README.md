```bash
# build the chessvm plugin, run e2e tests, and keep the network running
./scripts/build.release.sh \
&& VM_PLUGIN_PATH=$(pwd)/target/release/chessvm \
./scripts/tests.e2e.sh
```

```bash
cargo build \
  --release \
  --bin chessvm-cli
```

```bash
./target/release/chessvm-cli
```

## Acknowledgements

This project is inspired by and forks the
[TimestampVM](https://github.com/ava-labs/timestampvm-rs) project by Ava Labs.
