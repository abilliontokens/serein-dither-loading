//! Pre-renders the start screen. A plugin call gets 10M Wasm fuel, far too little to
//! rasterize a spin loop, so this script runs the plugin's own renderer over its meshes at
//! build time and writes every frame to `OUT_DIR`, with a generated `art.rs` describing
//! them. The library includes both; activation only base64-encodes the frames.

#[path = "src/mesh.rs"]
mod mesh;
#[path = "src/render.rs"]
mod render;

use std::{env, fmt::Write, fs, path::Path};

/// Frame rate of the spin loops.
const FPS: u8 = 12;
/// Yaw speed of every model in degrees per second: 5° a frame. Activation spends about
/// 0.1M fuel per frame on base64 and JSON escaping, so the longest loop (36 frames) needs
/// under 40% of a call's 10M.
const SPIN: f32 = 60.0;
/// Fixed pitch toward the viewer, in degrees.
const PITCH: f32 = 20.0;
/// Cells per side of the square model images.
const MODEL_SIDE: usize = 64;
/// Logical pixels per model cell.
const MODEL_CELL: u8 = 3;
/// The text below the model.
const TEXT: &str = "Loading...";
/// Image cells per glyph cell side, so the dither fills each glyph cell pixel by pixel.
const TEXT_SCALE: usize = 2;
/// Logical pixels per text cell, for 4-pixel glyph cells.
const TEXT_CELL: u8 = 2;
/// Transparent text cell rows above the line: Serein stacks images flush, so these keep
/// 16 pixels between the model and the text.
const TEXT_GAP: usize = 8;

/// A model the plugin offers.
struct Model {
	name: &'static str,
	mesh: fn() -> mesh::Mesh,
	/// The smallest yaw turn that maps the mesh onto itself, which its seamless loop covers.
	turn: f32,
}

/// The models in `next-model` order.
const MODELS: [Model; 3] = [
	Model {
		name: "spiky body",
		mesh: mesh::spiky_body,
		turn: 180.0,
	},
	Model {
		name: "torus knot",
		mesh: mesh::torus_knot,
		turn: 120.0,
	},
	Model {
		name: "crystal",
		mesh: mesh::crystal,
		turn: 60.0,
	},
];

fn main() {
	for path in ["build.rs", "src/mesh.rs", "src/render.rs"] {
		println!("cargo::rerun-if-changed={path}");
	}
	let out = env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR");
	let out = Path::new(&out);
	let mut art = format!("const FPS: u8 = {FPS};\n");
	let loading = render::text(TEXT, TEXT_SCALE, TEXT_GAP);
	let loading = emit(out, "loading", &loading, TEXT_CELL);
	writeln!(art, "const LOADING: Art = {loading};").expect("write to a String");
	writeln!(art, "const MODELS: [Model; {}] = [", MODELS.len()).expect("write to a String");
	for (index, Model { name, mesh, turn }) in MODELS.into_iter().enumerate() {
		let frames = turn * f32::from(FPS) / SPIN;
		assert_eq!(
			frames.fract(),
			0.0,
			"{name}: whole frames must cover its turn"
		);
		let image = render::spin(&mesh(), MODEL_SIDE, frames as usize, turn, PITCH);
		let image = emit(out, &format!("model-{index}"), &image, MODEL_CELL);
		writeln!(art, "\tModel {{ name: {name:?}, art: {image} }},").expect("write to a String");
	}
	art.push_str("];\n");
	fs::write(out.join("art.rs"), art).expect("write art.rs");
}

/// Writes `image`'s frames to `OUT_DIR/<stem>.cells` and returns the `Art` that includes them.
fn emit(out: &Path, stem: &str, image: &render::Image, cell: u8) -> String {
	fs::write(out.join(format!("{stem}.cells")), &image.frames).expect("write frames");
	format!(
		"Art {{ width: {}, height: {}, cell: {cell}, cells: include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{stem}.cells\")) }}",
		image.width, image.height
	)
}
