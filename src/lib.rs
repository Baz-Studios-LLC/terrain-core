//! The world generation Baz Studios games and Opificium's terrain bench both run.
//!
//! # Why this crate exists
//!
//! It was written twice, and the two copies had to agree exactly. A game and the
//! bench that shapes its ground both work the world out from scratch — nothing
//! but files pass between them — so a difference of one digit in a hash, or two
//! lines of a tree's own shaping swapped, gave the bench one world and the game
//! another. No error. Nothing failing. Just wrong.
//!
//! That was held together by tests pinning literal numbers copied out of one
//! program and asserted in the other. It worked, and it was a tax on every
//! change. Written once, the two cannot disagree at all.
//!
//! This is how the studios do it: an editor is built ON TOP of the game's own
//! runtime rather than beside it, and the world code exists once. Ours are
//! separate applications, so the shared part is a crate instead of a module —
//! but the principle is the same, and the alternative is what we had.
//!
//! # It names no engine
//!
//! Nothing here mentions Bevy. It cannot: the game and the bench are on
//! different Bevy majors and could not link the same one. It does not need to
//! either — Bevy's `Vec2` and `Vec3` are `glam`'s, re-exported, and everything
//! here is arithmetic over vectors.
//!
//! Geometry comes out as plain vertex arrays ([`Geometry`]), and each program
//! turns those into its own engine's mesh. That seam is a dozen lines on each
//! side and is the only engine-shaped thing in the whole arrangement.

pub mod biome;
pub mod cloud;
pub mod cover;
pub mod forest;
mod history;
pub mod painted;
pub mod river;
pub mod prop;
pub mod sculpt;
mod timber;
pub mod tree;

pub use glam::{Vec2, Vec3};

/// A patch of ground that changed, as a pair of corners: low, then high.
///
/// Not a rectangle type. Every engine has one and this crate names none, so the
/// two corners cross the boundary and each program turns them into whatever its
/// own is called — one line, at the one place that needs it.
pub type Patch = (Vec2, Vec2);

/// Hermite smoothstep: 0 below `edge0`, 1 above `edge1`, eased between.
///
/// Used everywhere a thing becomes another thing. Shared rather than copied
/// because a subtly different easing curve on either side would move every
/// coastline, every beach and every treeline by a little.
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if (edge1 - edge0).abs() < f32::EPSILON {
        return if x < edge0 { 0.0 } else { 1.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A mesh, before any engine has an opinion about it.
///
/// Positions, normals and texture coordinates as plain arrays, with indices into
/// them. Both programs build their own engine's mesh from this — the one seam
/// where the shared world meets a particular renderer.
#[derive(Default, Clone, PartialEq, Debug)]
pub struct Geometry {
    pub places: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    /// Linear RGBA per vertex, or empty when the mesh has none.
    ///
    /// Optional because most geometry here does not want it: a tree is coloured
    /// by the material its variety wears, which is cheaper and lets one mesh be
    /// planted a thousand times in a thousand shades. Ground cover cannot do
    /// that — a chunk's worth of grass is welded into ONE mesh, and one mesh
    /// wears one material, so a meadow's many greens have to live in its
    /// vertices.
    pub colours: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

impl Geometry {
    pub fn vertices(&self) -> usize {
        self.places.len()
    }

    /// Copies another geometry into this one: turned about its own upright,
    /// scaled, and stood at a place.
    ///
    /// # This is what welding is
    ///
    /// Fifty boulders on a hillside can be fifty entities wearing fifty
    /// transforms, or one mesh with the boulders already in the right places.
    /// The second is one draw call instead of fifty and nothing to keep in step
    /// as the world streams, and it is affordable exactly because these things
    /// carry their colour in their vertices — one mesh wears one material, so
    /// anything welded has to have given up on materials already.
    ///
    /// Turned about Y only. Everything here stands on the ground and would look
    /// wrong lying on its side, and a rotation about one axis is a sine and a
    /// cosine rather than a matrix.
    ///
    /// The scale is uniform for the same reason it usually is: a normal survives
    /// a uniform scale untouched, where a squashed one has to be re-derived
    /// through an inverse transpose and every lighting bug that follows from
    /// getting that wrong.
    pub fn stamp(&mut self, other: &Geometry, at: Vec3, turn: f32, scale: f32) {
        let base = self.places.len() as u32;
        let (sin, cos) = turn.sin_cos();
        let spin = |v: Vec3| Vec3::new(v.x * cos + v.z * sin, v.y, -v.x * sin + v.z * cos);

        for (index, place) in other.places.iter().enumerate() {
            let stood = spin(Vec3::from_array(*place)) * scale + at;
            self.places.push(stood.to_array());
            self.normals
                .push(spin(Vec3::from_array(other.normals[index])).to_array());
            self.uvs.push(other.uvs[index]);
            if let Some(colour) = other.colours.get(index) {
                self.colours.push(*colour);
            }
        }
        self.indices.extend(other.indices.iter().map(|i| i + base));
    }

    pub fn is_empty(&self) -> bool {
        self.places.is_empty()
    }
}

/// A ball: unit directions from its middle, and the triangles between them.
///
/// An octahedron split `splits` times and pushed out to the sphere. Callers scale
/// and squash the directions, and take each normal from the direction itself, so
/// what they draw shades round rather than breaking into flats.
///
/// # Shared, and INDEXED
///
/// Shared because the same shape was written twice and got it wrong the same way
/// twice: a bare octahedron with its vertices jittered is a SHARD. Clouds made of
/// them looked like broken glass, and so did the leaves.
///
/// Indexed because the first rounded version emitted three vertices per face and
/// cost twenty times what it needed to — one oak came to two hundred thousand
/// vertices. A split octahedron has only eighteen distinct corners at one split
/// and sixty-six at two; every one is shared by four to eight faces, and since
/// the normal is the direction, sharing them is not an approximation. Ninety-six
/// vertices became eighteen for exactly the same picture.
pub fn ball(splits: usize) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let mut points: Vec<Vec3> = vec![
        Vec3::X,
        Vec3::NEG_X,
        Vec3::Y,
        Vec3::NEG_Y,
        Vec3::Z,
        Vec3::NEG_Z,
    ];
    let mut faces: Vec<[u32; 3]> = vec![
        [0, 2, 4],
        [2, 1, 4],
        [1, 3, 4],
        [3, 0, 4],
        [2, 0, 5],
        [1, 2, 5],
        [3, 1, 5],
        [0, 3, 5],
    ];

    for _ in 0..splits {
        // Which midpoints have already been made, so an edge shared by two faces
        // makes one corner and not two. Without this the corners multiply and
        // nothing is shared at all — which is the whole cost being paid for.
        let mut midpoints: std::collections::HashMap<(u32, u32), u32> =
            std::collections::HashMap::new();
        let mut halfway = |points: &mut Vec<Vec3>, a: u32, b: u32| -> u32 {
            let key = if a < b { (a, b) } else { (b, a) };
            *midpoints.entry(key).or_insert_with(|| {
                let at = (points[a as usize] + points[b as usize]) * 0.5;
                points.push(at);
                points.len() as u32 - 1
            })
        };

        let mut finer = Vec::with_capacity(faces.len() * 4);
        for [a, b, c] in faces {
            let ab = halfway(&mut points, a, b);
            let bc = halfway(&mut points, b, c);
            let ca = halfway(&mut points, c, a);
            finer.push([a, ab, ca]);
            finer.push([ab, b, bc]);
            finer.push([ca, bc, c]);
            finer.push([ab, bc, ca]);
        }
        faces = finer;
    }

    // Out to the sphere last, so every corner sits at the same radius however
    // many times it was split.
    for point in &mut points {
        *point = point.normalize_or(Vec3::Y);
    }
    (points, faces)
}

/// A repeatable stream of numbers from one seed.
///
/// Hashed rather than drawn from a generator crate, so that a given seed gives
/// the same answer in both programs whatever else either happens to be asking
/// for numbers at the time. Every constant here is load-bearing.
pub struct Draw {
    state: u32,
}

impl Draw {
    pub fn new(seed: u32) -> Self {
        Self {
            state: seed ^ 0x9E37_79B9,
        }
    }

    pub fn unit(&mut self) -> f32 {
        let mut h = self.state;
        h ^= h >> 16;
        h = h.wrapping_mul(0x7feb_352d);
        h ^= h >> 15;
        h = h.wrapping_mul(0x846c_a68b);
        h ^= h >> 16;
        self.state = self.state.wrapping_add(0x9E37_79B9);
        h as f32 / u32::MAX as f32
    }

    pub fn between(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.unit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ball_is_round_and_shares_its_corners() {
        for (splits, corners, faces) in [(0, 6, 8), (1, 18, 32), (2, 66, 128)] {
            let (points, triangles) = ball(splits);
            assert_eq!(points.len(), corners, "{splits} splits");
            assert_eq!(triangles.len(), faces, "{splits} splits");

            // Every corner on the sphere, which is what makes it a ball rather
            // than a polyhedron with the middles of its edges caved in.
            for point in &points {
                assert!(
                    (point.length() - 1.0).abs() < 1.0e-5,
                    "{point:?} is off the sphere"
                );
            }
            // And every face names corners that exist.
            for face in &triangles {
                for corner in face {
                    assert!((*corner as usize) < points.len());
                }
            }
        }
    }

    #[test]
    fn splitting_shares_every_edge_it_makes() {
        // The point of the midpoint cache. Without it a split makes a new corner
        // per FACE rather than per edge, nothing is shared, and the count goes up
        // twentyfold — which is what put one oak at two hundred thousand
        // vertices.
        let (points, faces) = ball(2);
        assert!(
            points.len() < faces.len(),
            "a shared ball has fewer corners than faces: {} against {}",
            points.len(),
            faces.len()
        );
    }

    #[test]
    fn smoothstep_eases_between_its_edges() {
        assert_eq!(smoothstep(0.0, 1.0, -1.0), 0.0);
        assert_eq!(smoothstep(0.0, 1.0, 2.0), 1.0);
        assert_eq!(smoothstep(0.0, 1.0, 0.5), 0.5);
        // Backwards edges invert, which several callers rely on to fade OUT.
        assert_eq!(smoothstep(1.0, 0.0, 0.0), 1.0);
        // Equal edges must not divide by zero.
        assert_eq!(smoothstep(1.0, 1.0, 0.5), 0.0);
        assert_eq!(smoothstep(1.0, 1.0, 1.5), 1.0);
    }

    #[test]
    fn stamping_moves_a_shape_without_changing_it() {
        // Welding is only worth anything if what comes out the other side is the
        // same object. Two of them stamped in different places must be the same
        // shape, each at its own size, and both still facing outward.
        let mut one = Geometry::default();
        let (corners, faces) = ball(1);
        for out in &corners {
            one.places.push(out.to_array());
            one.normals.push(out.to_array());
            one.uvs.push([0.0, 0.0]);
            one.colours.push([0.5, 0.5, 0.5, 1.0]);
        }
        for face in &faces {
            one.indices.extend_from_slice(face);
        }

        let mut welded = Geometry::default();
        welded.stamp(&one, Vec3::new(10.0, 2.0, -4.0), 0.9, 3.0);
        welded.stamp(&one, Vec3::new(-30.0, 0.0, 7.0), 2.4, 0.5);

        assert_eq!(welded.vertices(), one.vertices() * 2);
        assert_eq!(welded.colours.len(), welded.places.len());
        assert_eq!(welded.indices.len(), one.indices.len() * 2);

        // The second copy's triangles must point at the second copy's vertices.
        let half = one.vertices() as u32;
        assert!(
            welded.indices[one.indices.len()..].iter().all(|i| *i >= half),
            "the second stamp is drawing the first one's vertices"
        );

        for (from, at, scale) in [
            (0, Vec3::new(10.0, 2.0, -4.0), 3.0),
            (one.vertices(), Vec3::new(-30.0, 0.0, 7.0), 0.5),
        ] {
            for index in from..from + one.vertices() {
                let out = Vec3::from_array(welded.places[index]) - at;
                assert!(
                    (out.length() - scale).abs() < 1.0e-4,
                    "a stamped vertex sits {:.4} from its middle, not {scale}",
                    out.length()
                );
                // And its normal turned with it. On a ball the normal points
                // straight out, so it has to still agree with where the vertex
                // went — which is the thing a rotation gets wrong quietly.
                let says = Vec3::from_array(welded.normals[index]);
                assert!(
                    says.dot(out.normalize()) > 0.999,
                    "a stamped normal no longer agrees with its own surface"
                );
            }
        }
    }

    #[test]
    fn a_seed_always_draws_the_same_numbers() {
        let drawn: Vec<f32> = (0..6).map(|_| Draw::new(42).unit()).collect();
        assert!(drawn.windows(2).all(|w| w[0] == w[1]), "same seed, same first draw");

        let mut draw = Draw::new(42);
        let run: Vec<f32> = (0..8).map(|_| draw.unit()).collect();
        assert!(run.iter().all(|v| (0.0..=1.0).contains(v)), "outside 0..1: {run:?}");
        // A stream that repeats itself would give every tree the same limb.
        assert!(run.windows(2).all(|w| w[0] != w[1]), "the stream stalled: {run:?}");
    }
}
