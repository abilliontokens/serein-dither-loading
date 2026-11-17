# Dither Loading for Serein

> **Preview SDK, not yet released.** This plugin needs a Serein build with the
> `start_screen` capability from [Serein PR #1042](https://github.com/ViceVerse-cz/Serein/pull/1042).
> Released clients do not have it yet.

A [Serein](https://github.com/ViceVerse-cz/Serein) extension that replaces the
start screen with a small splash: a lit, spinning low-poly model above
**Loading...** in dither pixel text, in the blocky Bayer-dithered look of
[dither-ui](https://dither-ui.com/docs/world/).

## How it renders

The plugin renders everything itself, and Serein only plays the frames. The
`start_screen` capability accepts pre-rendered cell images (2-bit cells colored
from a palette of up to three colors, one still frame or a loop at a frame
rate) and stacks them above Serein's unchanged progress bar and account button.
Serein has no font, model or dither of its own for this screen.

- `src/render.rs` is the renderer: a 5x9 pixel font whose glyph cells are filled
  cell by cell through a 4x4 ordered (Bayer) dither over a brightness ramp down
  each line, and a software mesh rasterizer (turntable fit, yaw spin, a fixed
  20° pitch, perspective, a depth buffer, Lambert plus ambient light) whose
  lighting the same dither prints as lit, shaded or empty cells.
- `src/mesh.rs` mints the models in code; no assets ship in the package.
- `build.rs` runs both at build time and writes every frame into the library.
  A plugin call gets 10M units of Wasm fuel, far too little to rasterize a spin
  loop, so activation only base64-encodes the stored frames and returns them.

| Model | Shape | Spin loop |
| --- | --- | --- |
| Spiky body | icosphere core with a sharp six-sided cone radiating from each of its 42 vertices, like the dither-ui world sample; 336 vertices, 332 triangles | 36 frames: a half turn maps it onto itself |
| Torus knot | trefoil (p=2, q=3) with an 8-sided tube; 768 vertices, 1536 triangles | 24 frames: a third of a turn |
| Crystal | hexagonal bipyramid; 8 vertices, 12 triangles | 12 frames: a sixth of a turn |

Each model is 64x64 cells of 3 pixels, spinning at 60°/s (5° a frame at 12
fps). Every loop covers the smallest turn that maps its model onto itself, so it
repeats seamlessly; the build fails if a loop's turn would not. *Loading...* is
one still image, 16 pixels below the model, with 4-pixel glyph cells each filled
by 2x2 dither cells. Lit model cells take the model color (soft violet and blue
tones chosen to read on a dark background), the shaded rest of each silhouette
and glyph the same color at 0.18 opacity, and lit text cells #EDEDEF.

The spiky body's activation output is about 50 KB of JSON (37 KB of packed
frames) and uses under 40% of the call's fuel; the other models need less.

## Grants

| Capability | Why |
| --- | --- |
| `start_screen` | Replace the start screen with the pre-rendered model and text, in a 360x480 splash window. |
| `storage` | Remember the on/off choice and the picked model. |

## Behavior

- **Customize the start screen** (activation) re-applies the saved choice
  whenever the plugin is enabled or Serein starts: on by default, spiky body
  first.
- While it starts, Serein shows a 360x480 splash window without its logo: the
  spinning model above *Loading...* in the dither pixel font, with no title or
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
