# gpui-pre-linux XIM patch

This repository contains the published `gpui-pre-linux` 0.3.5 snapshot of
Zed's Apache-2.0 `gpui_linux` crate at
`d89e9c2124b2786a390c7a451c7488601b4da2e1`, with a focused X11 XIM fix.
The initial commit imports the crates.io package unchanged. Modified files
carry a notice; the original license is preserved in `LICENSE-APACHE`.

The patch waits for input-method attributes and a valid native window before
creating an input context, reuses it when the same window regains focus, and
guards reset, key forwarding, and candidate-position updates until it is ready.
It keeps the package version and all other GPUI snapshot dependencies unchanged.

Consumers use a Cargo `[patch.crates-io]` entry pinned to the fix commit.

Run normal checks with `cargo test --features test-support --lib` and
`cargo clippy --features test-support --all-targets -- -D warnings`.
On an isolated X11 display with a running XIM server, run the real protocol
regressions with `cargo test --features test-support --lib xim_handler::tests
-- --ignored --test-threads=1`. They cover both startup orders and context reuse.
