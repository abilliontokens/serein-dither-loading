//! Low-poly meshes minted in code, so no assets ship in the package: a spiky body, a
//! trefoil torus knot and a crystal. Only the build script runs this, through
//! `render::spin`; the plugin itself carries just the rendered frames.

use std::collections::BTreeMap;
use std::f32::consts::TAU;

/// A triangle mesh: vertex positions `[x, y, z]`, y up, and triangles indexing them. The
/// renderer lights whichever side faces the camera.
pub struct Mesh {
	pub vertices: Vec<[f32; 3]>,
	pub triangles: Vec<[u16; 3]>,
}

/// The vertical axis the models spin about.
const UP: [f32; 3] = [0.0, 1.0, 0.0];

pub fn sum(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
	[a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
	[a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn scaled(vector: [f32; 3], factor: f32) -> [f32; 3] {
	[vector[0] * factor, vector[1] * factor, vector[2] * factor]
}

pub fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
	a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
	[
		a[1] * b[2] - a[2] * b[1],
		a[2] * b[0] - a[0] * b[2],
		a[0] * b[1] - a[1] * b[0],
	]
}

pub fn unit(vector: [f32; 3]) -> [f32; 3] {
	scaled(vector, 1.0 / dot(vector, vector).sqrt())
}

/// The world axis least aligned with `axis`, a reference never parallel to it.
fn least_aligned(axis: [f32; 3]) -> [f32; 3] {
	[[1.0, 0.0, 0.0], UP, [0.0, 0.0, 1.0]]
		.into_iter()
		.min_by(|a, b| dot(*a, axis).abs().total_cmp(&dot(*b, axis).abs()))
		.expect("three axes")
}

/// Push `sides` points on a circle of `radius` around `center`, counter-clockwise about the
/// unit `axis`, starting perpendicular to both `axis` and `reference`.
fn push_ring(
	vertices: &mut Vec<[f32; 3]>,
	center: [f32; 3],
	axis: [f32; 3],
	reference: [f32; 3],
	radius: f32,
	sides: usize,
) {
	// `first × second = axis`, so sweeping from `first` toward `second` turns about it.
	let second = unit(cross(axis, reference));
	let first = cross(second, axis);
	for side in 0..sides {
		let (sin, cos) = (side as f32 * TAU / sides as f32).sin_cos();
		let offset = sum(scaled(first, radius * cos), scaled(second, radius * sin));
		vertices.push(sum(center, offset));
	}
}

fn vertex_index(index: usize) -> u16 {
	u16::try_from(index).expect("vertex count fits u16")
}

/// The shared midpoint of edge `a`–`b`, pushed onto the unit sphere and minted once per
/// edge.
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
		vertex_index(vertices.len() - 1)
	})
}

/// Faceted unit icosphere: one subdivision of an icosahedron, 42 vertices and 80
/// counter-clockwise triangles. The vertical axis runs through two edge midpoints, so a
/// half turn about it maps the sphere onto itself.
fn icosphere() -> Mesh {
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

	// One subdivision: split every face at edge midpoints pushed onto the sphere, keeping
	// the outward counter-clockwise winding.
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

	Mesh {
		vertices,
		triangles,
	}
}

/// The dither-ui world sample's spiky body: an icosphere core with a sharp six-sided cone
/// radiating from each of its 42 vertices, 336 vertices and 332 triangles. Each cone's open
/// base sits just inside the core, so the core hides it. It keeps the icosphere's half-turn
/// symmetry about the vertical axis.
pub fn spiky_body() -> Mesh {
	const SIDES: usize = 6;
	const BASE: f32 = 0.9;
	const RADIUS: f32 = 0.17;
	const TIP: f32 = 1.8;
	let Mesh {
		mut vertices,
		mut triangles,
	} = icosphere();
	let core = vertices.len();
	vertices.reserve(core * (SIDES + 1));
	triangles.reserve(core * SIDES);
	for index in 0..core {
		let axis = vertices[index];
		let base = vertices.len();
		let center = scaled(axis, BASE);
		push_ring(
			&mut vertices,
			center,
			axis,
			least_aligned(axis),
			RADIUS,
			SIDES,
		);
		let tip = vertex_index(vertices.len());
		vertices.push(scaled(axis, TIP));
		for side in 0..SIDES {
			let current = vertex_index(base + side);
			let next = vertex_index(base + (side + 1) % SIDES);
			triangles.push([current, next, tip]);
		}
	}

	Mesh {
		vertices,
		triangles,
	}
}

/// Trefoil torus knot (p = 2, q = 3): 96 samples around the path carry an 8-sided tube,
/// 768 vertices and 1536 triangles. Every tube ring is framed against the vertical axis,
/// which the path never runs along, so the knot's three lobes are exact copies a third of
/// a turn apart.
pub fn torus_knot() -> Mesh {
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
	let mut vertices: Vec<[f32; 3]> = Vec::with_capacity(SAMPLES * RADIAL);
	for sample in 0..SAMPLES {
		let t = sample as f32 * step;
		let tangent = unit(sub(point(t + step), point(t - step)));
		push_ring(&mut vertices, point(t), tangent, UP, TUBE, RADIAL);
	}

	let index = |sample: usize, side: usize| vertex_index(sample * RADIAL + side);
	let mut triangles: Vec<[u16; 3]> = Vec::with_capacity(SAMPLES * RADIAL * 2);
	for sample in 0..SAMPLES {
		let next = (sample + 1) % SAMPLES;
		for side in 0..RADIAL {
			let following = (side + 1) % RADIAL;
			let a = index(sample, side);
			let b = index(next, side);
			let c = index(next, following);
			let d = index(sample, following);
			// Each ring turns counter-clockwise about the path's direction, so
			// outward-facing quads run against the path here.
			triangles.push([a, c, b]);
			triangles.push([a, d, c]);
		}
	}

	Mesh {
		vertices,
		triangles,
	}
}

/// A hexagonal bipyramid crystal: two apexes over a six-sided girdle, 8 vertices and 12
/// triangles, the same after every sixth of a turn.
pub fn crystal() -> Mesh {
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

	Mesh {
		vertices,
		triangles,
	}
}
