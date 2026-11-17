//! Dither Loading: replaces Serein's start (connecting) screen with a spinning low-poly model
//! above "Loading..." in dither pixel text, in a small splash window.
//!
//! The plugin renders everything itself; Serein only plays the frames back. Its renderer
//! (`render.rs`: a 5x9 pixel font, a 4x4 ordered (Bayer) dither and a mesh rasterizer) and
//! its meshes (`mesh.rs`, minted in code: a spiky body, a trefoil torus knot and a crystal)
//! run in `build.rs`, which pre-renders every frame as packed 2-bit cells, because a plugin
//! call's 10M Wasm fuel could not rasterize a spin loop. Activation returns those frames,
//! base64-encoded, as a declarative [`StartScreen`] through [`StartScreenOutput`]. The
//! panel's `next-model` button cycles the models and `toggle` restores the stock screen.
//! Choices persist through the `storage` grant, and corrupt storage falls back to the
//! defaults (on, first model).

use serein_extension_sdk::serde_json::{self, Value};
use serein_extension_sdk::{
	Element, Invocation, Output, StartImage, StartScreen, StartScreenOutput, StartWindow,
};

/// A pre-rendered image: `width × height` cells of `cell` logical pixels each, and its
/// packed 2-bit frames back to back.
struct Art {
	width: u16,
	height: u16,
	cell: u8,
	cells: &'static [u8],
}

/// A model's name and its seamless spin loop.
struct Model {
	name: &'static str,
	art: Art,
}

// `FPS`, `LOADING` and `MODELS`, rendered by build.rs.
include!(concat!(env!("OUT_DIR"), "/art.rs"));

/// How many models the plugin offers; `next-model` cycles through them.
const MODEL_COUNT: usize = MODELS.len();
/// Lit model cells use Serein's current accent, the color of its progress bar, so the model
/// matches the bar and follows the user's theme; the shade is the accent at 0x2E (about 0.18)
/// opacity.
const MODEL_PALETTE: [&str; 2] = ["accent", "accent:2E"];
/// Color of the lit text pixels.
const TEXT_COLOR: &str = "#EDEDEF";
/// The splash window: 244 pixels of images (the model, then the text 16 pixels below it)
/// above Serein's progress bar and account button, about 140 pixels with their padding,
/// leaves room for its header and margins, so the images keep their full size.
const WINDOW: StartWindow = StartWindow {
	width: 360,
	height: 480,
};
/// Standard base64 digits.
const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// The four base64 digits of a 24-bit group.
fn digits(group: u32) -> [u8; 4] {
	let digit = |shift: u32| BASE64[((group >> shift) & 63) as usize];
	[digit(18), digit(12), digit(6), digit(0)]
}

/// Standard padded base64 of `bytes`. Every Wasm operator costs fuel, so whole groups take
/// a straight-line path without per-byte loops or bounds checks.
fn base64(bytes: &[u8]) -> String {
	let mut text = vec![b'='; bytes.len().div_ceil(3) * 4];
	let (groups, tail) = bytes.as_chunks::<3>();
	let (quads, _) = text.as_chunks_mut::<4>();
	for (&[a, b, c], quad) in groups.iter().zip(quads.iter_mut()) {
		*quad = digits(u32::from_be_bytes([0, a, b, c]));
	}
	if let Some(quad) = quads.last_mut().filter(|_| !tail.is_empty()) {
		let mut group = [0; 4];
		group[1..=tail.len()].copy_from_slice(tail);
		quad[..=tail.len()].copy_from_slice(&digits(u32::from_be_bytes(group))[..=tail.len()]);
	}
	String::from_utf8(text).expect("base64 digits are ASCII")
}

/// One screen image of `art`'s frames; base64 is all the work they need.
fn image(art: &Art, palette: &[String]) -> StartImage {
	let frame = (usize::from(art.width) * usize::from(art.height)).div_ceil(4);
	StartImage {
		width: art.width,
		height: art.height,
		palette: palette.to_vec(),
		frames: art.cells.chunks(frame).map(base64).collect(),
		fps: FPS,
		cell: art.cell,
	}
}

/// The start screen with a given model: its spin loop above the still text. Both images
/// share one palette for the cell values the renderer prints: 1 lit, 2 shade, 3 text.
fn screen(model: usize) -> StartScreen {
	let palette = [
		MODEL_PALETTE[0].to_owned(),
		MODEL_PALETTE[1].to_owned(),
		TEXT_COLOR.to_owned(),
	];
	StartScreen {
		images: vec![
			image(&MODELS[model].art, &palette),
			image(&LOADING, &palette),
		],
		window: Some(WINDOW),
		..Default::default()
	}
}

/// Persisted choices. Corrupt or missing storage falls back to the default:
/// the custom screen on, first model.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Saved {
	off: bool,
	model: usize,
}

/// Strict decode: any missing piece keeps its default, any wrong-typed or
/// out-of-range field makes the whole value corrupt (defaults, unoverwritten).
fn saved(invocation: &Invocation) -> Saved {
	let fields = invocation
		.storage
		.as_deref()
		.and_then(|raw| serde_json::from_str::<Value>(raw).ok())
		.and_then(|value| value.as_object().cloned());
	let Some(fields) = fields else {
		return Saved::default();
	};
	let flag = |name: &str| -> Option<bool> {
		match fields.get(name) {
			None | Some(Value::Null) => Some(false),
			Some(value) => value.as_bool(),
		}
	};
	let model = match fields.get("model") {
		None | Some(Value::Null) => Some(0),
		Some(value) => value
			.as_u64()
			.filter(|model| usize::try_from(*model).is_ok_and(|model| model < MODEL_COUNT))
			.map(|model| model as usize),
	};
	match (flag("off"), model) {
		(Some(off), Some(model)) => Saved { off, model },
		_ => Saved::default(),
	}
}

fn storage_json(state: &Saved) -> String {
	serde_json::json!({"off": state.off, "model": state.model}).to_string()
}

/// This plugin's start screen for the saved state: `None` while off, which
/// the wire carries as an explicit `start_screen: null` to clear it.
fn current(state: &Saved) -> Option<StartScreen> {
	(!state.off).then(|| screen(state.model))
}

fn panel(state: &Saved) -> Vec<Element> {
	let text = if state.off {
		"Dither Loading is off: Serein shows its normal start screen while it \
		 starts. Turn it on for a splash window with a spinning model and \
		 dither pixel text instead."
	} else {
		"Dither Loading is on: while Serein starts, a small splash window shows \
		 a spinning model above Loading... in dither pixel text, without the \
		 logo. Turn it off to restore the normal start screen, or switch to the \
		 next model."
	};
	vec![
		Element::Text {
			text: format!("{text} Current model: {}.", MODELS[state.model].name),
		},
		Element::Button {
			id: "toggle".into(),
			label: if state.off { "Turn on" } else { "Turn off" }.into(),
		},
		Element::Button {
			id: "next-model".into(),
			label: "Next model".into(),
		},
	]
}

fn handle(input: Invocation) -> StartScreenOutput {
	let mut state = saved(&input);
	match input.action.as_str() {
		// Activation re-applies the saved choice; the host does not persist
		// storage returned from an activation, so nothing is written here.
		"activate" => StartScreenOutput {
			start_screen: Some(current(&state)),
			..Default::default()
		},
		"show" => StartScreenOutput {
			output: Output {
				panel: panel(&state),
				..Default::default()
			},
			..Default::default()
		},
		"toggle" => {
			state.off = !state.off;
			persisted(&state)
		}
		"next-model" => {
			state.model = (state.model + 1) % MODEL_COUNT;
			persisted(&state)
		}
		_ => StartScreenOutput::default(),
	}
}

/// Persist the changed choices and restate the start screen from them.
fn persisted(state: &Saved) -> StartScreenOutput {
	StartScreenOutput {
		output: Output {
			panel: panel(state),
			storage: Some(storage_json(state)),
			..Default::default()
		},
		start_screen: Some(current(state)),
	}
}

serein_extension_sdk::export!(handle);

#[cfg(test)]
mod tests {
	use super::*;
	use serein_extension_sdk::{MAX_IO_BYTES, dispatch_typed};

	fn wire(action: &str, storage: Option<&str>) -> Vec<u8> {
		let input = serde_json::json!({"action": action, "storage": storage}).to_string();
		dispatch_typed(input.as_bytes(), handle).expect("bounded invocation")
	}

	fn call(action: &str, storage: Option<&str>) -> Value {
		serde_json::from_slice(&wire(action, storage)).expect("wire JSON")
	}

	/// Which model a returned screen shows, by its loop length (every model's differs).
	fn model(value: &Value) -> usize {
		let frames = value["start_screen"]["images"][0]["frames"]
			.as_array()
			.map_or(0, Vec::len);
		(0..MODEL_COUNT)
			.find(|&index| {
				let art = &MODELS[index].art;
				art.cells.len() / (usize::from(art.width) * usize::from(art.height)).div_ceil(4)
					== frames
			})
			.expect("a known model")
	}

	/// Standard padded base64, decoded without the plugin's digit table.
	fn decode(text: &str) -> Vec<u8> {
		let digit = |byte: u8| {
			u32::from(match byte {
				b'A'..=b'Z' => byte - b'A',
				b'a'..=b'z' => byte - b'a' + 26,
				b'0'..=b'9' => byte - b'0' + 52,
				b'+' => 62,
				b'/' => 63,
				_ => panic!("not a base64 digit: {byte}"),
			})
		};
		text.as_bytes()
			.chunks(4)
			.flat_map(|chunk| {
				let digits = chunk.iter().take_while(|byte| **byte != b'=').count();
				let group = chunk[..digits]
					.iter()
					.enumerate()
					.fold(0, |group, (index, byte)| {
						group | (digit(*byte) << (18 - 6 * index))
					});
				(0..digits - 1).map(move |index| (group >> (16 - 8 * index)) as u8)
			})
			.collect()
	}

	#[test]
	fn every_model_emits_a_valid_screen_within_the_output_limit() {
		for (model, Model { name, .. }) in MODELS.iter().enumerate() {
			let storage = storage_json(&Saved { off: false, model });
			let bytes = wire("activate", Some(&storage));
			assert!(
				bytes.len() < MAX_IO_BYTES,
				"{name}: {} output bytes",
				bytes.len()
			);
			let output: StartScreenOutput = serde_json::from_slice(&bytes).expect("typed output");
			let screen = output.start_screen.flatten().expect("a start screen");
			screen
				.validate()
				.unwrap_or_else(|error| panic!("{name}: {error}"));
			assert_eq!(screen.window, Some(WINDOW));
			let [spin, text] = screen.images.as_slice() else {
				panic!("{name}: the spin loop and the text");
			};
			assert!(spin.frames.len() > 1, "{name} animates");
			assert_eq!(text.frames.len(), 1, "the text is still");
		}
	}

	#[test]
	fn spiky_body_first_frame_lights_its_center_and_leaves_its_corners_empty() {
		let value = call("activate", None);
		let model = &value["start_screen"]["images"][0];
		let side = model["width"].as_u64().expect("width") as usize;
		assert_eq!(model["height"].as_u64(), Some(side as u64));
		let frame = decode(model["frames"][0].as_str().expect("first frame"));
		assert_eq!(frame, MODELS[0].art.cells[..side * side / 4]);
		let cell = |x: usize, y: usize| {
			let index = y * side + x;
			(frame[index / 4] >> (6 - 2 * (index % 4))) & 3
		};
		let near = |center: usize| center - 4..center + 4;
		let lit = near(side / 2)
			.flat_map(|y| near(side / 2).map(move |x| (x, y)))
			.filter(|&(x, y)| cell(x, y) == 1)
			.count();
		assert!(lit > 0, "lit cells near the center");
		let edges = [0..8, side - 8..side];
		for rows in &edges {
			for columns in &edges {
				for y in rows.clone() {
					for x in columns.clone() {
						assert_eq!(cell(x, y), 0, "corner cell ({x}, {y}) is empty");
					}
				}
			}
		}
	}

	#[test]
	fn activation_replaces_the_start_screen() {
		let value = call("activate", None);
		let screen = &value["start_screen"];
		assert!(screen.get("title").is_none());
		assert!(screen.get("subtitle").is_none());
		assert_eq!(
			screen["window"],
			serde_json::json!({"width": 360, "height": 480})
		);
		let [model, text] = [&screen["images"][0], &screen["images"][1]];
		let palette = serde_json::json!(["accent", "accent:2E", "#EDEDEF"]);
		assert_eq!(model["palette"], palette);
		assert_eq!(text["palette"], palette);
		assert_eq!((&model["fps"], &model["cell"]), (&12.into(), &3.into()));
		assert_eq!(text["frames"].as_array().map(Vec::len), Some(1));
	}

	#[test]
	fn toggle_off_clears_and_toggle_on_restores() {
		let off = call("toggle", None);
		assert!(off.get("start_screen").is_some_and(Value::is_null));
		assert_eq!(off["panel"].as_array().map(Vec::len), Some(3));
		let storage = off["storage"].as_str().expect("persisted").to_string();

		let still_off = call("activate", Some(&storage));
		assert!(still_off.get("start_screen").is_some_and(Value::is_null));

		let on = call("toggle", Some(&storage));
		assert_eq!(model(&on), 0);
	}

	#[test]
	fn next_model_persists_and_cycles() {
		let knot = call("next-model", None);
		assert_eq!(model(&knot), 1);
		let storage = knot["storage"].as_str().expect("persisted").to_string();

		let crystal = call("next-model", Some(&storage));
		assert_eq!(model(&crystal), 2);
		let storage = crystal["storage"].as_str().expect("persisted").to_string();

		let spiky = call("next-model", Some(&storage));
		assert_eq!(model(&spiky), 0);
	}

	#[test]
	fn corrupt_storage_falls_back_to_defaults() {
		for corrupt in [
			"{oops",
			"{\"off\":\"yes\"}",
			"{\"model\":99}",
			"{\"model\":\"knot\"}",
			"[]",
		] {
			let value = call("activate", Some(corrupt));
			assert_eq!(model(&value), 0, "storage: {corrupt}");
		}
	}
}
