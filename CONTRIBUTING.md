# Contributing to Chaqmoq

Thanks for taking a look. Issues and pull requests are both welcome.

## Getting set up

You need a Rust toolchain (1.85 or newer). Nothing else: the `v8` crate
downloads a prebuilt static library, so there is no C++ toolchain, GN or Ninja
to install.

```bash
git clone https://github.com/ismoilovdevml/chaqmoq
cd chaqmoq
cargo build
cargo test --workspace
```

The first build compiles V8's Rust bindings and swc, which takes a few minutes.
Everything after that is fast.

## Before you open a pull request

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI runs the same three commands on Linux, macOS and Windows, plus every example
in `examples/`.

## Adding a runtime API

A new API touches three places, in this order:

1. **The op**, in `crates/chaqmoq-runtime/src/ops/`. It must start with a
   permission check — `state.borrow::<Permissions>().check_read(&path)?` or the
   equivalent for the capability it needs. An op that reaches the filesystem,
   the network or the environment without one is a security bug, not an
   oversight.
2. **The registration**, in the `deno_core::extension!` block in
   `crates/chaqmoq-runtime/src/lib.rs`. Ops are namespaced `op_chq_*` so they
   can never collide with one that `deno_core` installs.
3. **The JavaScript wrapper**, in `crates/chaqmoq-runtime/js/`. This is the
   surface users actually see, so it should follow the web platform wherever a
   relevant spec exists.

Then add the type signature to `types/chaqmoq.d.ts` and a test.

### A note on the JavaScript layer

Files under `crates/chaqmoq-runtime/js/` are embedded into the binary by
`include_js_files!`, which requires **7-bit ASCII**. Box-drawing characters and
similar must be written as `\uXXXX` escapes, and comments should use plain
ASCII punctuation. A non-ASCII byte fails at runtime startup, not at compile
time, so it is easy to miss.

Extension modules also load before the globals are installed, so they must
import what they need (`import { TextDecoder } from "ext:chaqmoq/01_web.js"`)
rather than reaching for `globalThis`.

## Tests

- **Unit tests** live next to the code they cover, in `#[cfg(test)]` modules.
- **End-to-end tests** are in `crates/chaqmoq-cli/tests/cli.rs`. They run the
  real binary against real files, which is the only way to cover permission
  errors, exit codes and stack-trace formatting.
- **Runtime behaviour** can also be tested in JavaScript, under
  `examples/tests/`, using `Chaqmoq.test`. These run as part of CI.

## Screenshots in the README

They are generated, never hand-edited:

```bash
./docs/tools/capture.sh
```

The script runs each command through a pseudo-terminal (so colour detection
behaves as it would for a human) and renders the captured output to SVG. If you
change output that appears in the README, re-run it and commit the result.

## Commit messages

Plain and descriptive is fine. Explain *why* in the body when the change is not
obvious from the diff.
