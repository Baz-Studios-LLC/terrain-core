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
const PUFFS: (f32, f32) = (5.0, 9.0);

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

/// One puff: an octahedron pushed out unevenly, shaded top to bottom.
fn blob(into: &mut Geometry, at: Vec3, size: Vec3, tall: f32, draw: &mut Draw) {
    const POINTS: [Vec3; 6] = [
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(-1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, -1.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(0.0, 0.0, -1.0),
    ];
    const FACES: [[u32; 3]; 8] = [
        [0, 2, 4],
        [2, 1, 4],
        [1, 3, 4],
        [3, 0, 4],
        [2, 0, 5],
        [1, 2, 5],
        [3, 1, 5],
        [0, 3, 5],
    ];

    let base = into.places.len() as u32;
    for point in POINTS {
        let out = point * size * draw.between(0.78, 1.22);
        let place = at + out;
        into.places.push(place.to_array());
        let normal = out.normalize_or(Vec3::Y);
        into.normals.push(normal.to_array());
        into.uvs.push([0.5, 0.5]);

        // Top to bottom across the cloud's own height, not the puff's, so one
        // lump shades as one lump rather than each puff shading separately.
        let up = (place.y / tall.max(0.001) * 0.5 + 0.5).clamp(0.0, 1.0);
        let shade = mix(UNDER, TOP, up);
        into.colours.push([shade[0], shade[1], shade[2], 1.0]);
    }
    for face in FACES {
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
