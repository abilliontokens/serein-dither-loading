//! The dither renderer behind every frame the plugin emits: a 5x9 pixel font and a software
//! mesh rasterizer, both printed through one 4x4 ordered (Bayer) dither into the start
//! screen's packed 2-bit cells. Only the build script runs it: a plugin call's Wasm fuel
//! could not rasterize a spin loop, so the plugin carries the finished frames instead.

use crate::mesh::{Mesh, cross, dot, scaled, sub, sum};

/// Cell value of a model surface the dither lights.
const LIT: u8 = 1;
/// Cell value of the faint rest of the model's silhouette and of each glyph.
const SHADE: u8 = 2;
/// Cell value of a glyph pixel the dither lights.
const INK: u8 = 3;
/// Brightness lost from the top to the bottom row of each glyph.
const TEXT_RAMP: f32 = 0.45;
/// Glyph rows: seven down to the baseline and two for descenders.
const GLYPH_ROWS: usize = 9;
/// Camera distance from the unit sphere's center; it keeps the sphere inside the field of view.
const CAMERA: f32 = 3.1;
/// Focal length of the 40-degree field of view: 1 / tan(20°).
const FOCAL: f32 = 2.747_477;
/// Unit direction toward the light, from the upper left in front of the model.
const LIGHT: [f32; 3] = [-0.453_4, 0.654_9, 0.604_5];
/// Light reaching faces turned away from the light source.
const AMBIENT: f32 = 0.22;

/// 4x4 ordered-dither thresholds, in sixteenths.
const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// 5x9 glyphs for ASCII 0x20 through 0x7E: seven rows down to the baseline and two descender
/// rows; each row's bit 4 is the leftmost column.
const GLYPHS: [[u8; 9]; 95] = [
	[0, 0, 0, 0, 0, 0, 0, 0, 0],                         // space
	[0x04, 0x04, 0x04, 0x04, 0x04, 0, 0x04, 0, 0],       // !
	[0x0A, 0x0A, 0, 0, 0, 0, 0, 0, 0],                   // "
	[0x0A, 0x0A, 0x1F, 0x0A, 0x1F, 0x0A, 0x0A, 0, 0],    // #
	[0x04, 0x0F, 0x14, 0x0E, 0x05, 0x1E, 0x04, 0, 0],    // $
	[0x18, 0x19, 0x02, 0x04, 0x08, 0x13, 0x03, 0, 0],    // %
	[0x0C, 0x12, 0x14, 0x08, 0x15, 0x12, 0x0D, 0, 0],    // &
	[0x04, 0x04, 0x08, 0, 0, 0, 0, 0, 0],                // '
	[0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02, 0, 0],    // (
	[0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08, 0, 0],    // )
	[0, 0x04, 0x15, 0x0E, 0x15, 0x04, 0, 0, 0],          // *
	[0, 0x04, 0x04, 0x1F, 0x04, 0x04, 0, 0, 0],          // +
	[0, 0, 0, 0, 0, 0x0C, 0x0C, 0x04, 0x08],             // ,
	[0, 0, 0, 0x1F, 0, 0, 0, 0, 0],                      // -
	[0, 0, 0, 0, 0, 0x0C, 0x0C, 0, 0],                   // .
	[0, 0x01, 0x02, 0x04, 0x08, 0x10, 0, 0, 0],          // /
	[0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E, 0, 0],    // 0
	[0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E, 0, 0],    // 1
	[0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F, 0, 0],    // 2
	[0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E, 0, 0],    // 3
	[0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02, 0, 0],    // 4
	[0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E, 0, 0],    // 5
	[0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E, 0, 0],    // 6
	[0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08, 0, 0],    // 7
	[0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E, 0, 0],    // 8
	[0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C, 0, 0],    // 9
	[0, 0x0C, 0x0C, 0, 0x0C, 0x0C, 0, 0, 0],             // :
	[0, 0x0C, 0x0C, 0, 0, 0x0C, 0x0C, 0x04, 0x08],       // ;
	[0x02, 0x04, 0x08, 0x10, 0x08, 0x04, 0x02, 0, 0],    // <
	[0, 0, 0x1F, 0, 0x1F, 0, 0, 0, 0],                   // =
	[0x08, 0x04, 0x02, 0x01, 0x02, 0x04, 0x08, 0, 0],    // >
	[0x0E, 0x11, 0x01, 0x02, 0x04, 0, 0x04, 0, 0],       // ?
	[0x0E, 0x11, 0x01, 0x0D, 0x15, 0x15, 0x0E, 0, 0],    // @
	[0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11, 0, 0],    // A
	[0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E, 0, 0],    // B
	[0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E, 0, 0],    // C
	[0x1C, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1C, 0, 0],    // D
	[0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F, 0, 0],    // E
	[0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10, 0, 0],    // F
	[0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F, 0, 0],    // G
	[0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11, 0, 0],    // H
	[0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E, 0, 0],    // I
	[0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C, 0, 0],    // J
	[0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11, 0, 0],    // K
	[0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F, 0, 0],    // L
	[0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11, 0, 0],    // M
	[0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0, 0],    // N
	[0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E, 0, 0],    // O
	[0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10, 0, 0],    // P
	[0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D, 0, 0],    // Q
	[0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11, 0, 0],    // R
	[0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E, 0, 0],    // S
	[0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0, 0],    // T
	[0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E, 0, 0],    // U
	[0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04, 0, 0],    // V
	[0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A, 0, 0],    // W
	[0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11, 0, 0],    // X
	[0x11, 0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0, 0],    // Y
	[0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F, 0, 0],    // Z
	[0x0E, 0x08, 0x08, 0x08, 0x08, 0x08, 0x0E, 0, 0],    // [
	[0, 0x10, 0x08, 0x04, 0x02, 0x01, 0, 0, 0],          // backslash
	[0x0E, 0x02, 0x02, 0x02, 0x02, 0x02, 0x0E, 0, 0],    // ]
	[0x04, 0x0A, 0x11, 0, 0, 0, 0, 0, 0],                // ^
	[0, 0, 0, 0, 0, 0, 0, 0x1F, 0],                      // _
	[0x08, 0x04, 0x02, 0, 0, 0, 0, 0, 0],                // `
	[0, 0, 0x0E, 0x01, 0x0F, 0x11, 0x0F, 0, 0],          // a
	[0x10, 0x10, 0x16, 0x19, 0x11, 0x11, 0x1E, 0, 0],    // b
	[0, 0, 0x0E, 0x10, 0x10, 0x11, 0x0E, 0, 0],          // c
	[0x01, 0x01, 0x0D, 0x13, 0x11, 0x11, 0x0F, 0, 0],    // d
	[0, 0, 0x0E, 0x11, 0x1F, 0x10, 0x0E, 0, 0],          // e
	[0x06, 0x09, 0x08, 0x1C, 0x08, 0x08, 0x08, 0, 0],    // f
	[0, 0, 0x0F, 0x11, 0x11, 0x11, 0x0F, 0x01, 0x0E],    // g
	[0x10, 0x10, 0x16, 0x19, 0x11, 0x11, 0x11, 0, 0],    // h
	[0x04, 0, 0x0C, 0x04, 0x04, 0x04, 0x0E, 0, 0],       // i
	[0x02, 0, 0x06, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C], // j
	[0x10, 0x10, 0x12, 0x14, 0x18, 0x14, 0x12, 0, 0],    // k
	[0x0C, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E, 0, 0],    // l
	[0, 0, 0x1A, 0x15, 0x15, 0x11, 0x11, 0, 0],          // m
	[0, 0, 0x16, 0x19, 0x11, 0x11, 0x11, 0, 0],          // n
	[0, 0, 0x0E, 0x11, 0x11, 0x11, 0x0E, 0, 0],          // o
	[0, 0, 0x1E, 0x11, 0x11, 0x11, 0x1E, 0x10, 0x10],    // p
	[0, 0, 0x0F, 0x11, 0x11, 0x11, 0x0F, 0x01, 0x01],    // q
	[0, 0, 0x16, 0x19, 0x10, 0x10, 0x10, 0, 0],          // r
	[0, 0, 0x0F, 0x10, 0x0E, 0x01, 0x1E, 0, 0],          // s
	[0x08, 0x08, 0x1C, 0x08, 0x08, 0x09, 0x06, 0, 0],    // t
	[0, 0, 0x11, 0x11, 0x11, 0x13, 0x0D, 0, 0],          // u
	[0, 0, 0x11, 0x11, 0x11, 0x0A, 0x04, 0, 0],          // v
	[0, 0, 0x11, 0x11, 0x15, 0x15, 0x0A, 0, 0],          // w
	[0, 0, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0, 0],          // x
	[0, 0, 0x11, 0x11, 0x11, 0x11, 0x0F, 0x01, 0x0E],    // y
	[0, 0, 0x1F, 0x02, 0x04, 0x08, 0x1F, 0, 0],          // z
	[0x02, 0x04, 0x04, 0x08, 0x04, 0x04, 0x02, 0, 0],    // {
	[0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0, 0],    // |
	[0x08, 0x04, 0x04, 0x02, 0x04, 0x04, 0x08, 0, 0],    // }
	[0, 0, 0x08, 0x15, 0x02, 0, 0, 0, 0],                // ~
];
/// Outline drawn for every character outside printable ASCII.
const MISSING: [u8; 9] = [0x1F, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1F, 0, 0];

/// One image's frames, back to back. Each frame holds `width × height` 2-bit cells
/// row-major, packed four per byte with the first cell in the two most significant bits and
/// the last byte zero-padded: the start screen's frame format.
pub struct Image {
	pub width: usize,
	pub height: usize,
	pub frames: Vec<u8>,
}

impl Image {
	fn new(width: usize, height: usize, frames: usize) -> Self {
		Self {
			width,
			height,
			frames: Vec::with_capacity(frames * (width * height).div_ceil(4)),
		}
	}

	/// Appends one frame of cell values.
	fn push(&mut self, cells: &[u8]) {
		self.frames.extend(cells.chunks(4).map(|chunk| {
			chunk
				.iter()
				.zip([6, 4, 2, 0])
				.fold(0, |byte, (cell, shift)| byte | (cell << shift))
		}));
	}
}

/// The ordered-dither threshold of a cell, between 0 and 1.
fn threshold(x: usize, y: usize) -> f32 {
	(f32::from(BAYER[y % 4][x % 4]) + 0.5) / 16.0
}

fn glyph(character: char) -> &'static [u8; 9] {
	match character {
		' '..='~' => &GLYPHS[character as usize - 0x20],
		_ => &MISSING,
	}
}

/// Prints one line in the pixel font below `gap` transparent rows, each glyph cell `scale`
/// cells square. Every glyph cell is filled cell by cell with the Bayer pattern over a
/// brightness ramp down the line, so glyph shapes stay whole while their fill dithers: lit
/// cells take the text color and the rest of each glyph a faint shade.
pub fn text(line: &str, scale: usize, gap: usize) -> Image {
	let columns = line.chars().count() * 6 - 1;
	let mut image = Image::new(columns * scale, gap + GLYPH_ROWS * scale, 1);
	let mut cells = vec![0; image.width * image.height];
	for (index, character) in line.chars().enumerate() {
		for (row, bits) in glyph(character).iter().enumerate() {
			let brightness = 1.0 - TEXT_RAMP * row as f32 / (GLYPH_ROWS - 1) as f32;
			for column in (0..5).filter(|column| bits & (0x10 >> column) != 0) {
				let (left, top) = ((index * 6 + column) * scale, row * scale);
				for y in top..top + scale {
					for x in left..left + scale {
						cells[(gap + y) * image.width + x] = if brightness > threshold(x, y) {
							INK
						} else {
							SHADE
						};
					}
				}
			}
		}
	}
	image.push(&cells);
	image
}

/// Renders `frames` evenly spaced turns of `mesh` covering `turn` degrees of yaw, each frame
/// `side` cells square with the mesh tilted `pitch` degrees toward the viewer. `turn` must
/// map the mesh onto itself, so the loop repeats seamlessly; this panics when it does not.
pub fn spin(mesh: &Mesh, side: usize, frames: usize, turn: f32, pitch: f32) -> Image {
	let vertices = fit(&mesh.vertices);
	assert!(
		symmetric(&vertices, &mesh.triangles, turn),
		"a {turn}° turn does not map the mesh onto itself, so its spin loop would jump"
	);
	let pitch = pitch.to_radians();
	let mut raster = Raster::new(side);
	let mut image = Image::new(side, side, frames);
	let mut cells = vec![0; side * side];
	for frame in 0..frames {
		let yaw = (turn * frame as f32 / frames as f32).to_radians();
		raster.draw(&vertices, &mesh.triangles, yaw, pitch);
		raster.print(&mut cells);
		image.push(&cells);
	}
	image
}

/// Whether a yaw of `turn` degrees maps every triangle onto one of `triangles`, matched by
/// centroid within rounding.
fn symmetric(vertices: &[[f32; 3]], triangles: &[[u16; 3]], turn: f32) -> bool {
	let (sin, cos) = turn.to_radians().sin_cos();
	let centroids: Vec<[f32; 3]> = triangles
		.iter()
		.map(|triangle| {
			let [a, b, c] = triangle.map(|index| vertices[usize::from(index)]);
			scaled(sum(sum(a, b), c), 1.0 / 3.0)
		})
		.collect();
	centroids.iter().all(|&[x, y, z]| {
		let turned = [x * cos + z * sin, y, z * cos - x * sin];
		centroids
			.iter()
			.any(|centroid| length(sub(turned, *centroid)) < 1e-4)
	})
}

/// `vertices` fitted to a turntable: the vertical axis through the origin, which every mesh
/// is minted around, stays the axis they spin about, so a symmetric mesh keeps its symmetry.
/// They are centered vertically on their bounds and scaled into the unit sphere, so every
/// turn fits the frame without clipping.
fn fit(vertices: &[[f32; 3]]) -> Vec<[f32; 3]> {
	let (low, high) = vertices
		.iter()
		.fold((f32::INFINITY, f32::NEG_INFINITY), |(low, high), vertex| {
			(low.min(vertex[1]), high.max(vertex[1]))
		});
	let center = [0.0, (low + high) * 0.5, 0.0];
	let radius = vertices
		.iter()
		.map(|vertex| length(sub(*vertex, center)))
		.fold(0.0, f32::max);
	let scale = if radius > 0.0 { radius.recip() } else { 1.0 };
	vertices
		.iter()
		.map(|vertex| scaled(sub(*vertex, center), scale))
		.collect()
}

/// A square cell grid holding the nearest surface's lighting per cell, reused across frames.
struct Raster {
	side: usize,
	/// Inverse view depth of the nearest surface; zero where the model leaves a cell empty.
	depth: Vec<f32>,
	light: Vec<f32>,
	view: Vec<[f32; 3]>,
}

impl Raster {
	fn new(side: usize) -> Self {
		Self {
			side,
			depth: vec![0.0; side * side],
			light: vec![0.0; side * side],
			view: Vec::new(),
		}
	}

	/// Draws the mesh turned by `yaw` about its vertical axis and tilted by `pitch` toward the
	/// viewer, with perspective, a depth buffer and two-sided Lambert plus ambient lighting.
	fn draw(&mut self, vertices: &[[f32; 3]], triangles: &[[u16; 3]], yaw: f32, pitch: f32) {
		self.depth.fill(0.0);
		let (yaw_sin, yaw_cos) = yaw.sin_cos();
		let (pitch_sin, pitch_cos) = pitch.sin_cos();
		self.view.clear();
		self.view.extend(vertices.iter().map(|&[x, y, z]| {
			let (x, z) = (x * yaw_cos + z * yaw_sin, z * yaw_cos - x * yaw_sin);
			[
				x,
				y * pitch_cos - z * pitch_sin,
				y * pitch_sin + z * pitch_cos,
			]
		}));
		let center = self.side as f32 * 0.5;
		let scale = center * FOCAL;
		for triangle in triangles {
			let [a, b, c] = triangle.map(|index| self.view[usize::from(index)]);
			let normal = cross(sub(b, a), sub(c, a));
			let size = length(normal);
			if size <= f32::EPSILON {
				continue;
			}
			// Light the side facing the camera, whatever the winding.
			let facing = dot(normal, [-a[0], -a[1], CAMERA - a[2]]).signum();
			let light = AMBIENT + (1.0 - AMBIENT) * (dot(normal, LIGHT) * facing / size).max(0.0);
			let project = |vertex: [f32; 3]| {
				let inverse = (CAMERA - vertex[2]).recip();
				[
					center + vertex[0] * inverse * scale,
					center - vertex[1] * inverse * scale,
					inverse,
				]
			};
			let points = [project(a), project(b), project(c)];
			let area = edge(points[0], points[1], [points[2][0], points[2][1]]);
			if area.abs() <= f32::EPSILON {
				continue;
			}
			let (left, right) = span(points.map(|point| point[0]), self.side);
			let (top, bottom) = span(points.map(|point| point[1]), self.side);
			for y in top..bottom {
				for x in left..right {
					let cell = [x as f32 + 0.5, y as f32 + 0.5];
					let weights = [
						edge(points[1], points[2], cell) / area,
						edge(points[2], points[0], cell) / area,
						edge(points[0], points[1], cell) / area,
					];
					if weights.iter().any(|weight| *weight < 0.0) {
						continue;
					}
					let depth = weights[0] * points[0][2]
						+ weights[1] * points[1][2]
						+ weights[2] * points[2][2];
					let index = y * self.side + x;
					if depth > self.depth[index] {
						self.depth[index] = depth;
						self.light[index] = light;
					}
				}
			}
		}
	}

	/// Thresholds each covered cell's lighting against the Bayer pattern: lit cells take the
	/// model color, the rest of the silhouette a faint shade, and uncovered cells stay empty.
	fn print(&self, cells: &mut [u8]) {
		for (index, cell) in cells.iter_mut().enumerate() {
			*cell = if self.depth[index] <= 0.0 {
				0
			} else if self.light[index] > threshold(index % self.side, index / self.side) {
				LIT
			} else {
				SHADE
			};
		}
	}
}

/// The cell range a projected coordinate span touches, clamped to the grid.
fn span(values: [f32; 3], limit: usize) -> (usize, usize) {
	let low = values.into_iter().fold(f32::INFINITY, f32::min);
	let high = values.into_iter().fold(f32::NEG_INFINITY, f32::max);
	(
		(low.floor().max(0.0) as usize).min(limit),
		(high.ceil().max(0.0) as usize).min(limit),
	)
}

/// Twice the signed area of the triangle `a`, `b`, `p` in screen space.
fn edge(a: [f32; 3], b: [f32; 3], p: [f32; 2]) -> f32 {
	(b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}

fn length(a: [f32; 3]) -> f32 {
	dot(a, a).sqrt()
}
