//! Dither Loading: replaces Serein's start (connecting) screen with "Loading"
//! in dither pixel text and a procedurally minted spinning model.
//!
//! The plugin draws nothing. Activation returns a declarative [`StartScreen`]
//! through [`StartScreenOutput`]; the host keeps the last accepted value and
//! renders the text and the mesh through its ordered-dither (Bayer) raster
//! before any account data loads. The mesh is minted in code (no assets): a
//! faceted planet with a tilted ring, a trefoil torus knot, or a crystal, in
//! that order; the panel's `next-model` button cycles them and `toggle`
//! restores the stock screen. Choices persist through the `storage` grant and
//! corrupt storage falls back to the defaults (on, first model).

use std::collections::BTreeMap;
use std::f32::consts::TAU;

use serein_extension_sdk::serde_json::{self, Value};
use serein_extension_sdk::{
	Element, Invocation, Output, StartModel, StartScreen, StartScreenOutput, StartTextStyle,
};

/// How many models the plugin mints; `next-model` cycles through them.
const MODEL_COUNT: usize = 3;
/// Yaw speed in degrees per second; a touch faster than the dither-ui default.
const SPIN: f32 = 24.0;
/// Fixed pitch toward the viewer in degrees.
const PITCH: f32 = 20.0;
/// Palette chosen to read on a dark background.
const COLORS: [&str; MODEL_COUNT] = ["#B79CFF", "#8FB7FF", "#D3B8FF"];

/// Names shown in the settings panel.
fn model_name(index: usize) -> &'static str {
	match index {
		0 => "ringed planet",
		1 => "torus knot",
		_ => "crystal",
	}
}

/// The mesh for a model index: geometry minted in code, presentation set once.
fn mesh(index: usize) -> StartModel {
	let (vertices, triangles) = match index {
		0 => planet_with_ring(),
		1 => torus_knot(),
		_ => crystal(),
	};
	StartModel {
		vertices,
		triangles,
		color: Some(COLORS[index].into()),
		spin: SPIN,
		pitch: PITCH,
	}
}

/// The start screen the plugin asks for with a given model.
fn screen(model: usize) -> StartScreen {
	StartScreen {
		title: Some("Loading".into()),
		subtitle: Some("Connecting to Discord".into()),
		text_style: StartTextStyle::Dither,
		hide_logo: true,
		model: Some(mesh(model)),
	}
}

fn sum(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
	[a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scaled(vector: [f32; 3], factor: f32) -> [f32; 3] {
	[vector[0] * factor, vector[1] * factor, vector[2] * factor]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
	a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
	[
		a[1] * b[2] - a[2] * b[1],
		a[2] * b[0] - a[0] * b[2],
		a[0] * b[1] - a[1] * b[0],
	]
}

fn unit(vector: [f32; 3]) -> [f32; 3] {
	scaled(vector, 1.0 / dot(vector, vector).sqrt())
}

/// The shared midpoint of edge `a`–`b`, pushed onto the unit sphere and
/// minted once per edge.
fn midpoint(
	a: u16,
	b: u16,
	vertices: &mut Vec<[f32; 3]>,
	cache: &mut BTreeMap<[u16; 2], u16>,
) -> u16 {
	let key = [a.min(b), a.max(b)];
	*cache.entry(key).or_insert_with(|| {
		let middle = sum(vertices[usize::from(a)], vertices[usize::from(b)]);
		vertices.push(unit(middle));
		u16::try_from(vertices.len() - 1).expect("vertex count fits u16")
	})
}

/// Faceted planet: an icosphere (one subdivision of an icosahedron, 42
/// vertices, 80 triangles) plus a tilted annulus ring, emitted with both
/// windings so the raster shows it from either side.
fn planet_with_ring() -> (Vec<[f32; 3]>, Vec<[u16; 3]>) {
	const SEGMENTS: usize = 16;
	const RING_TILT: f32 = 0.55;
	let ratio = 0.5 * (1.0 + 5.0_f32.sqrt());
	let mut vertices: Vec<[f32; 3]> = [
		[-1.0, ratio, 0.0],
		[1.0, ratio, 0.0],
		[-1.0, -ratio, 0.0],
		[1.0, -ratio, 0.0],
		[0.0, -1.0, ratio],
		[0.0, 1.0, ratio],
		[0.0, -1.0, -ratio],
		[0.0, 1.0, -ratio],
		[ratio, 0.0, -1.0],
		[ratio, 0.0, 1.0],
		[-ratio, 0.0, -1.0],
		[-ratio, 0.0, 1.0],
	]
	.into_iter()
	.map(unit)
	.collect();
	let faces: Vec<[u16; 3]> = [
		(0, 11, 5),
		(0, 5, 1),
		(0, 1, 7),
		(0, 7, 10),
		(0, 10, 11),
		(1, 5, 9),
		(5, 11, 4),
		(11, 10, 2),
		(10, 7, 6),
		(7, 1, 8),
		(3, 9, 4),
		(3, 4, 2),
		(3, 2, 6),
		(3, 6, 8),
		(3, 8, 9),
		(4, 9, 5),
		(2, 4, 11),
		(6, 2, 10),
		(8, 6, 7),
		(9, 8, 1),
	]
	.into_iter()
	.map(|(a, b, c)| [a, b, c])
	.collect();

	// One subdivision: split every face at edge midpoints pushed onto the
	// sphere, keeping the outward counter-clockwise winding.
	let mut cache = BTreeMap::new();
	let mut triangles: Vec<[u16; 3]> = Vec::with_capacity(faces.len() * 4);
	for [a, b, c] in faces {
		let ab = midpoint(a, b, &mut vertices, &mut cache);
		let bc = midpoint(b, c, &mut vertices, &mut cache);
		let ca = midpoint(c, a, &mut vertices, &mut cache);
		triangles.push([a, ab, ca]);
		triangles.push([b, bc, ab]);
		triangles.push([c, ca, bc]);
		triangles.push([ab, bc, ca]);
	}

	// The ring: an annulus around the equator, tilted like a ringed planet.
	let ring_base = vertices.len();
	let (tilt_sin, tilt_cos) = RING_TILT.sin_cos();
	for step in 0..SEGMENTS {
		let angle = step as f32 * TAU / SEGMENTS as f32;
		let (sin, cos) = angle.sin_cos();
		for radius in [1.55_f32, 2.3] {
			// The annulus lies in the x-z plane, then rotates around the x axis.
			vertices.push([
				radius * cos,
				-radius * sin * tilt_sin,
				radius * sin * tilt_cos,
			]);
		}
	}
	for step in 0..SEGMENTS {
		let following = (step + 1) % SEGMENTS;
		let current = (ring_base + step * 2) as u16;
		let next = (ring_base + following * 2) as u16;
		let (inner, outer) = (current, current + 1);
		let (inner_next, outer_next) = (next, next + 1);
		// Top and bottom windings, so the ring reads from both sides.
		triangles.push([inner, outer, outer_next]);
		triangles.push([inner, outer_next, inner_next]);
		triangles.push([inner, outer_next, outer]);
		triangles.push([inner, inner_next, outer_next]);
	}

	(vertices, triangles)
}

/// Trefoil torus knot (p = 2, q = 3): 96 samples around the path carry an
/// 8-sided tube with parallel-ish frames.
fn torus_knot() -> (Vec<[f32; 3]>, Vec<[u16; 3]>) {
	const SAMPLES: usize = 96;
	const RADIAL: usize = 8;
	const CARRIER: f32 = 2.0;
	const TUBE: f32 = 0.31;
	const SCALE: f32 = 0.5;
	const P: f32 = 2.0;
	const Q: f32 = 3.0;

	let point = |t: f32| -> [f32; 3] {
		let carrier = CARRIER + (Q * t).cos();
		[
			carrier * (P * t).cos() * SCALE,
			(Q * t).sin() * SCALE * 1.6,
			carrier * (P * t).sin() * SCALE,
		]
	};
	let step = TAU / SAMPLES as f32;
	let centers: Vec<[f32; 3]> = (0..SAMPLES)
		.map(|sample| point(sample as f32 * step))
		.collect();

	let mut vertices: Vec<[f32; 3]> = Vec::with_capacity(SAMPLES * RADIAL);
	for (sample, center) in centers.iter().enumerate() {
		let t = sample as f32 * step;
		let tangent = unit(sum(point(t + step), scaled(point(t - step), -1.0)));
		// The axis least aligned with the tangent anchors a stable frame.
		let reference = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
			.into_iter()
			.min_by(|a, b| dot(*a, tangent).abs().total_cmp(&dot(*b, tangent).abs()))
			.expect("three axes");
		let bitangent = unit(cross(tangent, reference));
		let normal = cross(bitangent, tangent);
		for side in 0..RADIAL {
			let angle = side as f32 * TAU / RADIAL as f32;
			let offset = sum(
				scaled(normal, TUBE * angle.cos()),
				scaled(bitangent, TUBE * angle.sin()),
			);
			vertices.push(sum(*center, offset));
		}
	}

	let index =
		|sample: usize, side: usize| u16::try_from(sample * RADIAL + side).expect("fits u16");
	let mut triangles: Vec<[u16; 3]> = Vec::with_capacity(SAMPLES * RADIAL * 2);
	for sample in 0..SAMPLES {
		let next = (sample + 1) % SAMPLES;
		for side in 0..RADIAL {
			let following = (side + 1) % RADIAL;
			let a = index(sample, side);
			let b = index(next, side);
			let c = index(next, following);
			let d = index(sample, following);
			// The frame turns from `normal` toward `bitangent` about the
			// tangent, so outward-facing quads run against the path here.
			triangles.push([a, c, b]);
			triangles.push([a, d, c]);
		}
	}

	(vertices, triangles)
}

/// A hexagonal bipyramid crystal: two apexes over a six-sided girdle.
fn crystal() -> (Vec<[f32; 3]>, Vec<[u16; 3]>) {
	const SIDES: usize = 6;
	let mut vertices: Vec<[f32; 3]> = vec![[0.0, 1.35, 0.0]];
	for side in 0..SIDES {
		let angle = side as f32 * TAU / SIDES as f32;
		vertices.push([0.72 * angle.cos(), 0.0, 0.72 * angle.sin()]);
	}
	vertices.push([0.0, -1.35, 0.0]);
	let top = 0_u16;
	let bottom = SIDES as u16 + 1;
	let mut triangles: Vec<[u16; 3]> = Vec::with_capacity(SIDES * 2);
	for side in 0..SIDES as u16 {
		let current = side + 1;
		let following = (side + 1) % SIDES as u16 + 1;
		triangles.push([top, following, current]);
		triangles.push([bottom, current, following]);
	}

	(vertices, triangles)
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
		 connects. Turn it on for dither pixel text and a spinning model instead."
	} else {
		"Dither Loading is on: while Serein connects, the screen hides the logo \
		 and shows Loading in dither pixel text with a spinning model. Turn it \
		 off to restore the normal start screen, or switch to the next model."
	};
	vec![
		Element::Text {
			text: format!("{text} Current model: {}.", model_name(state.model)),
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
	use serein_extension_sdk::{
		MAX_START_MODEL_COORDINATE, MAX_START_MODEL_TRIANGLES, MAX_START_MODEL_VERTICES,
		dispatch_typed,
	};

	fn call(action: &str, storage: Option<&str>) -> Value {
		let input = serde_json::json!({"action": action, "storage": storage}).to_string();
		let bytes = dispatch_typed(input.as_bytes(), handle).expect("bounded invocation");
		serde_json::from_slice(&bytes).expect("wire JSON")
	}

	fn triangles(value: &Value) -> Option<usize> {
		value["start_screen"]["model"]["triangles"]
			.as_array()
			.map(Vec::len)
	}

	/// Signed volume; positive when closed surfaces wind counter-clockwise
	/// seen from outside, as the SDK contract requires.
	fn signed_volume(model: &StartModel) -> f32 {
		let at = |index: u16| model.vertices[usize::from(index)];
		model
			.triangles
			.iter()
			.map(|&[a, b, c]| dot(at(a), cross(at(b), at(c))) / 6.0)
			.sum()
	}

	#[test]
	fn every_model_validates_within_bounds() {
		// (vertices, triangles) per model; the README's table quotes these.
		let sizes = [(74, 144), (768, 1536), (8, 12)];
		for (index, size) in sizes.into_iter().enumerate() {
			let screen = screen(index);
			screen
				.validate()
				.unwrap_or_else(|error| panic!("model {index} ({}): {error}", model_name(index)));
			let model = screen.model.expect("screen carries a model");
			assert_eq!((model.vertices.len(), model.triangles.len()), size);
			assert!(model.vertices.len() <= MAX_START_MODEL_VERTICES);
			assert!(model.triangles.len() <= MAX_START_MODEL_TRIANGLES);
			assert!(
				model
					.vertices
					.iter()
					.flatten()
					.all(|value| value.is_finite() && value.abs() <= MAX_START_MODEL_COORDINATE)
			);
			assert!(signed_volume(&model) > 0.0, "model {index} winds inward");
		}
	}

	#[test]
	fn activation_replaces_the_start_screen() {
		let value = call("activate", None);
		assert_eq!(value["start_screen"]["title"], "Loading");
		assert_eq!(value["start_screen"]["subtitle"], "Connecting to Discord");
		assert_eq!(value["start_screen"]["text_style"], "dither");
		assert_eq!(value["start_screen"]["hide_logo"], true);
		assert_eq!(value["start_screen"]["model"]["color"], "#B79CFF");
		assert_eq!(value["start_screen"]["model"]["spin"], 24.0);
		assert_eq!(value["start_screen"]["model"]["pitch"], 20.0);
		assert!(
			value["start_screen"]["model"]["vertices"]
				.as_array()
				.is_some_and(|list| !list.is_empty())
		);
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
		assert_eq!(on["start_screen"]["title"], "Loading");
	}

	#[test]
	fn next_model_persists_and_cycles() {
		let knot = call("next-model", None);
		assert_eq!(triangles(&knot), Some(mesh(1).triangles.len()));
		let storage = knot["storage"].as_str().expect("persisted").to_string();

		let crystal = call("next-model", Some(&storage));
		assert_eq!(triangles(&crystal), Some(mesh(2).triangles.len()));
		let storage = crystal["storage"].as_str().expect("persisted").to_string();

		let planet = call("next-model", Some(&storage));
		assert_eq!(triangles(&planet), Some(mesh(0).triangles.len()));
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
			assert_eq!(
				value["start_screen"]["title"], "Loading",
				"storage: {corrupt}"
			);
			assert_eq!(triangles(&value), Some(mesh(0).triangles.len()));
		}
	}
}
