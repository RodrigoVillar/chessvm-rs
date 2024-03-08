# ChessVM-RS

> An Rust-based implementation of a Virtual Machine designed for the
> decentralized hosting and playing of Chess games

## Prerequisites

-   Rust must be already installed
-   Protoc must be already installed (for Mac users using Homebrew, run the
    following command: `brew install protobuf`)

## Spinning Up ChessVM

To deploy a local instance of ChessVM, run the following in your terminal:

```bash
# build the chessvm plugin, run e2e tests, and keep the network running
./scripts/build.release.sh \
&& VM_PLUGIN_PATH=$(pwd)/target/release/chessvm \
./scripts/tests.e2e.sh
```

If all goes well, you should see the following message:

```bash
Recommended HTTP-RPC: http://127.0.0.1:9656
Recommended URL-Path: ext/bc/7kPBUWKQDvAY8jEsGUuz4RaUj8GhrnjWgWhJeakQmMvjzrTUo/rpc
test tests::start_network ... ok
```

At this point, an Avalanche subnet with ChessVM bootstrapped is now running in
the background. If you want to interact with your instance of ChessVM via
ChessVM-CLI, please save the information above!

## Interacting with ChessVM via ChessVM-CLI

To get started with ChessVM-CLI, go to the root directory of this repository and
execute the following:

```bash
cargo build \
  --release \
  --bin chessvm-cli
```

If all goes well, the binary `chessvm-cli` should now exist under
`./target/release/chessvm-cli`. To check that you binary is executing correctly,
run the following command:

```bash
./target/release/chessvm-cli --help
```

You should see the following:

```bash
./target/release/chessvm-cli --help

A CLI to interact with an existing ChessVM instance

Usage: chessvm-cli -h <http-rpc> -u <url-path> [COMMAND]

Commands:
  ping             Checks if the given instance of ChessVM is running
  does-game-exist  Returns true if a game with the associated ID exists, false otherwise
  create-game      Creates a new Chess Game
  get-game         Returns FEN representation of the associated game if it exists
  make-move        Creates a transaction for the move
  help             Print this message or the help of the given subcommand(s)

Options:
  -h <http-rpc>
  -u <url-path>
  -h, --help         Print help
  -V, --version      Print version
```

To avoid having to pass in the `http-rpc`, `url-path` arguments every time you
make a ChessVM-CLI command, simply export the HTTP-RPC, URL-PATH variables you
were given as environment variables:

```bash
export HTTP_RPC="http://127.0.0.1:9656"
export URL_PATH="ext/bc/7kPBUWKQDvAY8jEsGUuz4RaUj8GhrnjWgWhJeakQmMvjzrTUo/rpc"
```

To test that you are able to succesfully interact with your instance of ChessVM,
you can ping the server (using the recommended HTTP-RPC/URL-Path variables you
were given when deploying ChessVM):

```bash
./target/release/chessvm-cli -h "http://127.0.0.1:9650" -u "ext/bc/2Qi9MXGenu8FxAPKjZqCjd7ev9QwFETTzdM7HeV9uV7cmfUR1K/rpc" ping

Response is true
```

Congrats; at this point, you're ready to interact with ChessVM!

### Gameplay Commands

To create a new game (where the first address is the white player, and the
second address is the black player):

```bash
./target/release/chessvm-cli -h "http://127.0.0.1:9650" -u "ext/bc/2Qi9MXGenu8FxAPKjZqCjd7ev9QwFETTzdM7HeV9uV7cmfUR1K/rpc" create-game 0x7f610402ccc4CC1BEbcE9699819200f5f28ED6e3
0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045

Created Chess Game with ID: 17000072326831680876
```

To check if a game exists:

```bash
./target/release/chessvm-cli -h "http://127.0.0.1:9650" -u "ext/bc/2Qi9MXGenu8FxAPKjZqCjd7ev9QwFETTzdM7HeV9uV7cmfUR1K/rpc" does-game-exist 17000072326831680876

Response is true
```

To get the state of a game:

```bash
./target/release/chessvm-cli -h "http://127.0.0.1:9650" -u "ext/bc/2Qi9MXGenu8FxAPKjZqCjd7ev9QwFETTzdM7HeV9uV7cmfUR1K/rpc" get-game 17000072326831680876

Current game board is the following: rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR
```

To make a move:

```bash
./target/release/chessvm-cli -h "http://127.0.0.1:9650" -u "ext/bc/2Qi9MXGenu8FxAPKjZqCjd7ev9QwFETTzdM7HeV9uV7cmfUR1K/rpc" make-move normal 0x7f610402ccc4CC1BEbcE9699819200f5f28ED6e3 17000072326831680876 P e2 e4

Normal Move Transaction Submission Status: true
```

Note: To capture a piece, append the piece that you wish to capture to your make-move command (in FEN notation)

Getting the updated game state:

```bash
./target/release/chessvm-cli -h "http://127.0.0.1:9650" -u "ext/bc/2Qi9MXGenu8FxAPKjZqCjd7ev9QwFETTzdM7HeV9uV7cmfUR1K/rpc" get-game 17000072326831680876

Current game board is the following: rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR
```

At any point, using the `--help` flag in the CLI will give more details about
what a command does.

### Common Errors

Below is a list of common errors that you may run into while working with
ChessVM:

_Already Bootstrapped Error_:

If this error occurs, you should see the following error message:

```bash
thread 'tests::start_network' panicked at 'failed start: Custom { kind: Other, error: "failed stop 'status: Unknown, message: \"already bootstrapped\", details: [], metadata: MetadataMap { headers: {\"content-type\": \"application/grpc\"} }'" }', tests/e2e/src/tests/mod.rs:475:10
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
```

This implies that an instance of ChessVM is already running in the background;
we will need to kill the associated process in order to boot up a new instance
of ChessVM. If you scroll up in your terminal, you should see the following:

```bash
[2024-02-07T14:50:53Z INFO  e2e::tests] RUN THESE COMMANDS IF THE TESTS FAIL
[2024-02-07T14:50:53Z INFO  e2e::tests] pkill -P 35673 || true
[2024-02-07T14:50:53Z INFO  e2e::tests] kill -2 35673 || true
```

Execute the `pkill` and `kill` commands printed in your terminal; these commands
will kill the processes associated with the already existing instance of
ChessVM. After executing these commands, you should be able to deploy a new
instance of ChessVM without any issue.

## Acknowledgements

This project is inspired by and forks the
[TimestampVM](https://github.com/ava-labs/timestampvm-rs) project by Ava Labs.
