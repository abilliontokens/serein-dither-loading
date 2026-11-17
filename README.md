# Dither Loading for Serein

> **Preview SDK, not yet released.** This plugin needs a Serein build with the
> `start_screen` capability from [Serein PR #1042](https://github.com/ViceVerse-cz/Serein/pull/1042).
> Released clients do not have it yet.

A [Serein](https://github.com/ViceVerse-cz/Serein) extension that replaces the
start screen with a compact splash: **Loading...** in dither pixel text — the
blocky Bayer-dithered look of [dither-ui](https://dither-ui.com/docs/world/) —
above a lit, spinning low-poly model. The plugin draws nothing: it returns a
declarative start screen and Serein renders the text and mesh through its
ordered-dither raster before any account data loads.

The models are minted in code (no assets ship in the package):

| Model | Shape | Size |
| --- | --- | --- |
| Spiky body | icosphere core with a sharp six-sided cone radiating from each of its 42 vertices, like the dither-ui world sample | 336 vertices, 332 triangles |
| Torus knot | trefoil (p=2, q=3) with an 8-sided tube | 768 vertices, 1536 triangles |
| Crystal | hexagonal bipyramid | 8 vertices, 12 triangles |

Each spins at 24°/s with a 20° pitch, in soft violet and blue tones chosen to
read on a dark background.

## Grants

| Capability | Why |
| --- | --- |
| `start_screen` | Replace the start screen's text and spinning model and show it as a compact splash. |
| `storage` | Remember the on/off choice and the picked model. |

## Behavior

- **Customize the start screen** (activation) re-applies the saved choice
  whenever the plugin is enabled or Serein starts: on by default, spiky body
  first.
- While it starts, Serein shows a compact splash window without its logo:
  *Loading...* in the dither pixel font above the spinning model, with no
  subtitle. The window's previous size and position return afterwards.
- **Dither Loading settings** explains the current state and offers
  **Turn on/off** and **Next model**; the panel-list actions do the same.
- Toggling off returns an explicit `start_screen: null`, restoring Serein's
  own start screen immediately; the choice is remembered for the next start.
- With nothing saved, the custom screen is on with the first model. Corrupt
  storage falls back to that default and is overwritten on the next change.

## Build and install

The SDK dependency points at a local Serein checkout in `Cargo.toml`
(`path = "../Serein/extensions/sdk"`); the release pins a git revision.

```sh
rustup target add wasm32-unknown-unknown
cargo test --locked
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo build --locked --release --target wasm32-unknown-unknown
python pack.py manifest.json target/wasm32-unknown-unknown/release/dither_loading.wasm packages/dither-loading.serein-extension
```

`pack.py` is Serein's packaging script (`extensions/pack.py`), copied
unchanged. A prebuilt package is committed at
`packages/dither-loading.serein-extension`.

Import it in **Settings > Extensions**, review and grant the capabilities
above, then enable the plugin.

## License

MIT OR Apache-2.0, matching Serein.
