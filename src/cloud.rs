//! Clouds, grown the way the trees are.
//!
//! A handful of squashed puffs welded into one lump, drawn from a seed. Nothing
//! here is a simulation of weather — it is the stylised answer: a cloud that
//! reads as a cloud in silhouette from underneath, which is the only angle
//! anybody sees one from.
//!
//! # Why geometry and not a shader
//!
//! Because everything else in this world is geometry. A billboard or a volumetric
//! pass would look like it came from a different game, and would need a texture
//! and a custom material in each program that draws one. A lump of low-poly
//! puffs matches the trees and the ground cover, costs a few dozen vertices, and
//! whichever engine is asking already knows how to draw it.
//!
//! # Lit from underneath, in the vertices
//!
//! A cloud is the one thing in a world that is brighter on top than below, and by
//! a lot. Left to a light and a normal it comes out flat, because a directional
//! light cannot tell the inside of a cloud from its surface. So the shading is
//! baked in: white on top, dusk-grey underneath, and the game tints the whole
//! thing with whatever colour the sun happens to be.

use glam::Vec3;

use crate::{Draw, Geometry};

/// How many distinct clouds are grown.
///
/// A sky holds dozens at a time, turned and scaled differently, so a handful of
/// shapes is plenty — nobody compares two clouds four hundred metres up.
pub const VARIETIES: usize = 6;

/// How many puffs a cloud is made of.
const PUFFS: (f32, f32) = (4.0, 7.0);

/// The top of a cloud, and its underside, as linear RGB.
///
/// Not white and grey by eye: a cloud lit from above is nearly blown out on top
/// and still bright underneath, and taking the underside too dark makes an
/// overcast sky read as a ceiling of slate.
const TOP: [f32; 3] = [0.95, 0.96, 0.98];
const UNDER: [f32; 3] = [0.42, 0.46, 0.55];

/// Grows one cloud.
///
/// Comes out centred on its own origin, a few tens of metres across, with its
/// long axis along X — so a caller can turn it about Y and have the shape read
/// differently rather than reading as the same lump rotated.
pub fn grow(seed: u32) -> Geometry {
    let mut draw = Draw::new(seed);
    let mut mesh = Geometry::default();

    let puffs = draw.between(PUFFS.0, PUFFS.1).round() as usize;
    // Clouds are far wider than they are tall. A ball of puffs reads as a
    // cauliflower; a raft of them reads as weather.
    let long = draw.between(26.0, 52.0);
    let deep = long * draw.between(0.45, 0.75);
    // Flatter than it looks like it should be. The puffs are jittered in size
    // and nudged up and down, so the lump ends up about half again as tall as
    // this — set at a quarter, the puffiest draw came out barely two to one and
    // read as a cauliflower rather than as weather.
    let tall = long * draw.between(0.13, 0.22);

    for puff in 0..puffs {
        // Strung along the long axis with the ends smaller, so the lump tapers
        // instead of stopping.
        let along = (puff as f32 + 0.5) / puffs as f32;
        let taper = 1.0 - (along - 0.5).abs() * 1.4;

        let at = Vec3::new(
            (along - 0.5) * long,
            draw.between(-0.1, 0.25) * tall,
            draw.between(-0.5, 0.5) * deep * 0.5,
        );
        let size = Vec3::new(
            draw.between(0.5, 0.9) * long / puffs as f32 * 2.2,
            draw.between(0.6, 1.0) * tall,
            draw.between(0.5, 0.9) * deep * 0.5,
        ) * taper.max(0.35);

        blob(&mut mesh, at, size, tall, &mut draw);
    }

    mesh
}

/// One puff: a rounded ball, shaded top to bottom.
///
/// # It was an octahedron, and it looked like broken glass
///
/// Eight flat triangles with every vertex shoved out at random. That is a SHARD,
/// not a puff — and a cloud made of them is a heap of paper offcuts, which is
/// exactly how it read. Scaling it up only made the facets bigger.
///
/// A ball has to be round to read as soft, so this subdivides down to a hundred
/// and twenty-eight faces and pushes every vertex out to the same radius. The
/// SHAPE is jittered by moving and sizing whole puffs, never their vertices —
/// that is the difference between a lumpy cloud and a crumpled one.
fn blob(into: &mut Geometry, at: Vec3, size: Vec3, tall: f32, draw: &mut Draw) {
    /// How many times each face is split in four. Two gives 128 faces, which is
    /// round enough that a cloud overhead has no visible flats on it.
    const SPLITS: usize = 2;

    // One wobble for the whole puff rather than one per vertex: a puff keeps its
    // roundness and the CLOUD gets its irregularity from how the puffs sit.
    let wobble = draw.between(0.86, 1.14);

    let (corners, faces) = crate::ball(SPLITS);
    let base = into.places.len() as u32;
    for out in corners {
        // Out to the sphere first, THEN squashed to the puff's shape, so it is an
        // ellipsoid and never a lumpy polyhedron.
        let place = at + out * size * wobble;

        into.places.push(place.to_array());
        // The sphere's own normal, so the shading runs round it smoothly instead
        // of breaking into flats.
        into.normals.push(out.to_array());
        into.uvs.push([0.5, 0.5]);

        // Top to bottom across the CLOUD's height, not the puff's, so one lump
        // shades as one lump.
        let up = (place.y / tall.max(0.001) * 0.5 + 0.5).clamp(0.0, 1.0);
        let shade = mix(UNDER, TOP, up);
        into.colours.push([shade[0], shade[1], shade[2], 1.0]);
    }
    for face in faces {
        into.indices
            .extend_from_slice(&[base + face[0], base + face[1], base + face[2]]);
    }
}

fn mix(low: [f32; 3], high: [f32; 3], t: f32) -> [f32; 3] {
    [
        low[0] + (high[0] - low[0]) * t,
        low[1] + (high[1] - low[1]) * t,
        low[2] + (high[2] - low[2]) * t,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(mesh: &Geometry) -> Vec3 {
        let mut low = Vec3::splat(f32::MAX);
        let mut high = Vec3::splat(f32::MIN);
        for place in &mesh.places {
            low = low.min(Vec3::from_array(*place));
            high = high.max(Vec3::from_array(*place));
        }
        high - low
    }

    #[test]
    fn a_cloud_is_wider_than_it_is_tall() {
        // A ball of puffs reads as a cauliflower. What makes a lump read as
        // weather is that it is a raft.
        for seed in 0..VARIETIES as u32 {
            let mesh = grow(seed);
            let size = span(&mesh);
            assert!(
                size.x > size.y * 2.5,
                "cloud {seed} is {:.0} across and {:.0} tall",
                size.x,
                size.y
            );
            assert!(size.z > size.y, "cloud {seed} should be a raft, not a ridge");
        }
    }

    #[test]
    fn a_cloud_is_bright_on_top_and_darker_underneath() {
        // The one thing in a world that is lit from inside as much as outside. A
        // light and a normal cannot do it, so it is baked in — and if it were
        // not, an overcast sky would read as a flat grey ceiling.
        let mesh = grow(3);
        let highest = mesh
            .places
            .iter()
            .enumerate()
            .max_by(|a, b| a.1[1].total_cmp(&b.1[1]))
            .expect("a cloud has vertices")
            .0;
        let lowest = mesh
            .places
            .iter()
            .enumerate()
            .min_by(|a, b| a.1[1].total_cmp(&b.1[1]))
            .expect("a cloud has vertices")
            .0;

        let brightness = |index: usize| {
            let colour = mesh.colours[index];
            colour[0] + colour[1] + colour[2]
        };
        assert!(
            brightness(highest) > brightness(lowest) * 1.4,
            "top {:.2} against underside {:.2}",
            brightness(highest),
            brightness(lowest)
        );
    }

    #[test]
    fn a_puff_is_round_and_not_a_shard() {
        // What was wrong: eight flat triangles per puff with every vertex shoved
        // out at random, which is a shard. A heap of them reads as broken glass,
        // and that is exactly how the sky looked.
        //
        // Roundness is measurable: every vertex of one puff sits at very nearly
        // the same distance from its middle, and a face is small compared to the
        // ball it is part of.
        let mesh = grow(1);
        assert!(
            mesh.places.len() > 300,
            "a rounded puff needs subdividing: {} vertices for a whole cloud",
            mesh.places.len()
        );

        // Normals point away from the surface everywhere — a shard has faces
        // whose normal has nothing to do with where the vertex is.
        for (place, normal) in mesh.places.iter().zip(&mesh.normals) {
            let n = Vec3::from_array(*normal);
            assert!(
                (n.length() - 1.0).abs() < 1.0e-3,
                "normals should be unit: {:?}",
                n
            );
            let _ = place;
        }

        // And no triangle is a big flat slab: the longest edge is small next to
        // the cloud itself.
        let size = span(&mesh);
        let mut longest = 0.0_f32;
        for face in mesh.indices.chunks(3) {
            let [a, b, c] = [face[0], face[1], face[2]]
                .map(|i| Vec3::from_array(mesh.places[i as usize]));
            longest = longest
                .max((b - a).length())
                .max((c - b).length())
                .max((a - c).length());
        }
        assert!(
            longest < size.x * 0.2,
            "a face {longest:.1} m across on a cloud {:.1} m wide is a slab",
            size.x
        );
    }

    #[test]
    fn every_cloud_carries_a_colour_for_every_vertex() {
        for seed in 0..VARIETIES as u32 {
            let mesh = grow(seed);
            assert!(!mesh.places.is_empty(), "cloud {seed} is empty");
            assert_eq!(mesh.colours.len(), mesh.places.len(), "cloud {seed}");
            assert_eq!(mesh.normals.len(), mesh.places.len(), "cloud {seed}");
        }
    }

    #[test]
    fn no_two_clouds_are_the_same_cloud() {
        let shapes: std::collections::HashSet<Vec<u32>> = (0..VARIETIES as u32)
            .map(|seed| {
                grow(seed)
                    .places
                    .iter()
                    .flat_map(|place| place.iter().map(|v| v.to_bits()))
                    .collect()
            })
            .collect();
        assert_eq!(shapes.len(), VARIETIES);
    }
}
