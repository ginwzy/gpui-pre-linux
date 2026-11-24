# gpui-pre-linux XIM patch

This branch contains the published `gpui-pre-linux` 0.3.12 snapshot of
Zed's Apache-2.0 `gpui_linux` crate at
`d2a4fb6ded7c470be25d1541281da3882495b65b`, with the XIM patches ported from
0.3.5. The original license is preserved in `LICENSE-APACHE`.

The patch waits for input-method attributes and a valid native window before
creating an input context, reuses it when the same window regains focus, and
guards reset, key forwarding, and candidate-position updates until it is ready.
XFixes selection-owner changes reconnect the server named by `XMODIFIERS=@im=<name>`.
The companion `zed-xim` patch releases the transport window on disconnect.
X11 and Wayland both accept candidate-position updates during composition.

Consumers patch both packages with Cargo's `[patch.crates-io]`. Standalone
checks use the adjacent `../zed-xim` checkout and published GPUI 0.3.12.

Run normal checks with `cargo test --features test-support --lib --jobs 4 --
--test-threads=4` and `cargo clippy --features test-support --all-targets --jobs 4`.
The published 0.3.12 snapshot already emits Clippy warnings in the X11 clipboard
and power tests.
On an isolated X11 display with a running XIM server, run the real protocol
regressions with `cargo test --features test-support --lib --jobs 4 xim_handler::tests
-- --ignored --test-threads=4`. They cover both focus/handshake orders and
context reuse.
