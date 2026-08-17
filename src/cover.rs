//! Ground cover: grass, flowers, and the dry scrub that stands in for both.
//!
//! The small stuff. A world can have good ground, good trees and good weather
//! and still read as a model of a world, because nothing grows on it — what
//! makes ground look like ground at walking height is that it is *littered*.
//!
//! # Merged, not scattered as things
//!
//! A tree is an entity: there are thousands and each is a separate mesh handle,
//! which an engine batches. Grass is HUNDREDS of thousands, and an entity apiece
//! would sink the frame rate before anything was drawn. So a patch of cover comes
//! out as ONE mesh — every tuft in a chunk welded together — and that is why the
//! colour lives in the vertices here rather than in a material the way a tree's
//! does. One mesh can only wear one material, and a meadow is not one green.
//!
//! # Where it grows is the biome's business
//!
//! Which is the whole reason [`crate::biome`] exists. Grass on grassland, less of
//! it under a wood, dry scrub in the desert, tufts on a shore, and nothing at all
//! on bare rock, in the snow, in open water or on ground somebody has levelled
//! and walks on.

use glam::{Vec2, Vec3};

use crate::biome::Biome;
use crate::Geometry;

/// Metres between slots on the cover lattice.
///
/// Every slot gets at most one tuft, so this sets the ceiling on how much there
/// can ever be: a 128 m chunk holds about two thousand slots at this spacing.
/// Tightening it multiplies the vertex count by the square, so it is the first
/// number to look at if the frame rate drops.
pub const SPACING: f32 = 2.6;

/// What is growing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sprig {
    /// A tuft of blades. Most of everything.
    Grass,
    /// A tuft with a head of colour on it.
    Flower,
    /// Stiff, dry, splayed. What passes for grass where there is no water.
    Scrub,
}

/// How much cover a place carries, 0 to 1.
///
/// `sureness` is [`Biome::confidence`] — how strongly the ground reads as its own
/// kind. Cover fades with it rather than stopping at a boundary, so a meadow
/// thins into a wood instead of ending along a line.
pub fn density(biome: Biome, sureness: f32) -> f32 {
    let most = match biome {
        // Open country is what grass is for.
        Biome::Grass => 0.85,
        // Thinner under a canopy, because less light reaches the floor.
        Biome::Forest => 0.45,
        // Dry country carries something, and not much of it.
        Biome::Desert => 0.22,
        // Marram and sea grass, in clumps.
        Biome::Shore => 0.3,
        // Bare by definition, both of them.
        Biome::Rock | Biome::Snow => 0.05,
        // Nothing grows in water, and a town's ground is walked on.
        Biome::Water | Biome::Settled => 0.0,
    };
    // Never the full amount right at a boundary: cover is the first thing to
    // thin out as ground stops being what it was.
    most * (0.55 + 0.45 * sureness)
}

/// What grows here, given a roll for variety.
///
/// Flowers are a fraction of the grass rather than a thing of their own: a meadow
/// is grass with flowers IN it, and scattering them on their own lattice put them
/// in tidy rows of their own.
pub fn kind(biome: Biome, roll: f32) -> Sprig {
    match biome {
        Biome::Desert => Sprig::Scrub,
        // Rock and snow carry the odd hardy tuft and never a flower.
        Biome::Rock | Biome::Snow => Sprig::Grass,
        Biome::Shore => {
            if roll < 0.08 {
                Sprig::Flower
            } else {
                Sprig::Grass
            }
        }
        // A meadow is about a twelfth flowers; a wood floor rather fewer.
        Biome::Forest => {
            if roll < 0.05 {
                Sprig::Flower
            } else {
                Sprig::Grass
            }
        }
        _ => {
            if roll < 0.09 {
                Sprig::Flower
            } else {
                Sprig::Grass
            }
        }
    }
}

/// A repeatable 0..1 from a slot and a purpose.
///
/// The forest's hash, with its own salts. Shared deliberately: two programs
/// drawing the same meadow must scatter it identically, and one hash with
/// separate salts is how the woods already manage it.
pub fn chance(x: i32, z: i32, salt: u32) -> f32 {
    crate::forest::chance(x, z, salt)
}

/// Salts the cover uses. Numbered clear of the forest's, which owns 1 to 7.
pub const SALT_JITTER_X: u32 = 11;
pub const SALT_JITTER_Z: u32 = 12;
pub const SALT_PRESENT: u32 = 13;
pub const SALT_KIND: u32 = 14;
pub const SALT_TURN: u32 = 15;
pub const SALT_SCALE: u32 = 16;
pub const SALT_SHADE: u32 = 17;
pub const SALT_PETAL: u32 = 18;

/// The greens grass is drawn between, darkest first, as linear RGB.
///
/// The palette lives here rather than in a game because the colour is baked into
/// the vertices, and the vertices are made here. A tree can be tinted by its
/// material afterwards; a welded meadow cannot.
const GRASS_DARK: [f32; 3] = [0.055, 0.14, 0.035];
const GRASS_LIGHT: [f32; 3] = [0.16, 0.30, 0.075];
/// And the dry pair, for scrub.
const SCRUB_DARK: [f32; 3] = [0.16, 0.14, 0.06];
const SCRUB_LIGHT: [f32; 3] = [0.30, 0.26, 0.11];

/// What a flower head can be. Sparse enough that a handful of hues reads as
/// variety rather than as a pattern.
const PETALS: [[f32; 3]; 5] = [
    [0.62, 0.60, 0.55],  // white
    [0.60, 0.50, 0.05],  // yellow
    [0.42, 0.10, 0.28],  // pink
    [0.14, 0.14, 0.42],  // blue
    [0.46, 0.16, 0.06],  // orange
];

/// How tall a tuft stands, in metres, before its own scale is applied.
const HEIGHT: f32 = 0.42;

/// Adds one tuft to a mesh being built.
///
/// `turn` spins it about its own base, `scale` sizes it, and `shade` and `petal`
/// are rolls for its colour. Everything is appended in place, because a chunk's
/// worth of these is one mesh and building it is one loop.
pub fn add(into: &mut Geometry, kind: Sprig, at: Vec3, turn: f32, scale: f32, shade: f32, petal: f32) {
    let (dark, light, blades) = match kind {
        Sprig::Grass => (GRASS_DARK, GRASS_LIGHT, 4),
        Sprig::Flower => (GRASS_DARK, GRASS_LIGHT, 2),
        // Splayed and stiff, and more of them: a scrub clump is what a dry place
        // has instead of a sward.
        Sprig::Scrub => (SCRUB_DARK, SCRUB_LIGHT, 5),
    };

    let green = mix(dark, light, shade);
    let tall = HEIGHT * scale;
    let wide = 0.035 * scale;

    for blade in 0..blades {
        // Spread evenly round the tuft and leaned out, so a tuft is a tuft and
        // not a sheaf. Scrub leans further, which is what makes it look dry.
        let angle = turn + blade as f32 / blades as f32 * std::f32::consts::TAU;
        let lean = if kind == Sprig::Scrub { 0.55 } else { 0.3 };
        let out = Vec2::new(angle.cos(), angle.sin());
        // A little shorter each way round, so no tuft is a neat rosette.
        let length = tall * (0.75 + 0.25 * fract(shade + blade as f32 * 0.37));

        let base = at + Vec3::new(out.x, 0.0, out.y) * wide * 0.5;
        let tip = at + Vec3::new(out.x * lean * length, length, out.y * lean * length);
        let across = Vec3::new(-out.y, 0.0, out.x) * wide;

        // Darker at the root and lighter at the tip: the cheapest thing that
        // stops a field of blades reading as flat paint.
        blade_into(into, base - across, base + across, tip, shade_of(green, 0.6), green);
    }

    if kind == Sprig::Flower {
        // A head of three petals on top. Not a real flower and not trying to be:
        // at walking height what registers is a spot of colour above the grass.
        let colour = PETALS[(petal * PETALS.len() as f32) as usize % PETALS.len()];
        let head = at + Vec3::Y * tall * 0.92;
        let span = 0.055 * scale;
        for petal_index in 0..3 {
            let angle = turn + petal_index as f32 / 3.0 * std::f32::consts::TAU;
            let out = Vec3::new(angle.cos(), 0.0, angle.sin());
            let across = Vec3::new(-out.z, 0.0, out.x) * span * 0.5;
            blade_into(
                into,
                head - across,
                head + across,
                head + out * span + Vec3::Y * span * 0.35,
                colour,
                colour,
            );
        }
    }
}

/// One triangle: two points at the foot and one at the tip, coloured at each end.
fn blade_into(
    into: &mut Geometry,
    left: Vec3,
    right: Vec3,
    tip: Vec3,
    foot_colour: [f32; 3],
    tip_colour: [f32; 3],
) {
    let base = into.places.len() as u32;
    // Upright rather than surface-true. A blade's own normal points sideways, so
    // lighting it honestly makes a meadow flicker dark as the camera turns; facing
    // them up lights the field like the ground it belongs to.
    let up = [0.0, 1.0, 0.0];

    for (place, colour) in [(left, foot_colour), (right, foot_colour), (tip, tip_colour)] {
        into.places.push(place.to_array());
        into.normals.push(up);
        into.uvs.push([0.5, 0.5]);
        into.colours.push([colour[0], colour[1], colour[2], 1.0]);
    }
    // Both faces, because a blade is one triangle and half a meadow would
    // otherwise be missing from any given angle.
    into.indices
        .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 1]);
}

fn mix(low: [f32; 3], high: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        low[0] + (high[0] - low[0]) * t,
        low[1] + (high[1] - low[1]) * t,
        low[2] + (high[2] - low[2]) * t,
    ]
}

fn shade_of(colour: [f32; 3], by: f32) -> [f32; 3] {
    [colour[0] * by, colour[1] * by, colour[2] * by]
}

fn fract(value: f32) -> f32 {
    value - value.floor()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_grows_where_nothing_should() {
        for barren in [Biome::Water, Biome::Settled] {
            assert_eq!(
                density(barren, 1.0),
                0.0,
                "{} should carry no cover",
                barren.name()
            );
        }
        // Rock and snow are nearly bare rather than wholly: the odd hardy tuft
        // is what tells a mountainside from a rendering of one.
        for lean in [Biome::Rock, Biome::Snow] {
            let some = density(lean, 1.0);
            assert!((0.0..0.1).contains(&some), "{}: {some}", lean.name());
        }
    }

    #[test]
    fn open_country_carries_the_most_and_a_wood_carries_less() {
        let meadow = density(Biome::Grass, 1.0);
        let under_trees = density(Biome::Forest, 1.0);
        let dry = density(Biome::Desert, 1.0);
        assert!(meadow > under_trees, "{meadow} vs {under_trees}");
        assert!(under_trees > dry, "{under_trees} vs {dry}");
    }

    #[test]
    fn cover_thins_toward_a_boundary_rather_than_stopping_at_it() {
        // A meadow should fade into a wood, not end along a line somebody can
        // see from the air.
        let sure = density(Biome::Grass, 1.0);
        let edge = density(Biome::Grass, 0.0);
        assert!(edge < sure, "{edge} should be under {sure}");
        assert!(edge > 0.0, "and not nothing at all");
    }

    #[test]
    fn a_desert_grows_scrub_and_never_a_flower() {
        for step in 0..64 {
            let roll = step as f32 / 63.0;
            assert_eq!(kind(Biome::Desert, roll), Sprig::Scrub);
            assert_ne!(kind(Biome::Rock, roll), Sprig::Flower);
            assert_ne!(kind(Biome::Snow, roll), Sprig::Flower);
        }
    }

    #[test]
    fn a_meadow_is_mostly_grass_with_flowers_in_it() {
        let flowers = (0..1000)
            .filter(|step| kind(Biome::Grass, *step as f32 / 999.0) == Sprig::Flower)
            .count();
        // Some, and nowhere near most. A field of flowers is a different thing.
        assert!((30..200).contains(&flowers), "{flowers} in a thousand");
    }

    #[test]
    fn a_tuft_stands_on_the_ground_and_reaches_up_from_it() {
        let mut mesh = Geometry::default();
        add(&mut mesh, Sprig::Grass, Vec3::ZERO, 0.0, 1.0, 0.5, 0.0);

        assert!(!mesh.places.is_empty(), "a tuft should have geometry");
        assert_eq!(
            mesh.colours.len(),
            mesh.places.len(),
            "every vertex needs its colour, or the mesh is refused"
        );

        let lowest = mesh.places.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
        let highest = mesh.places.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
        assert!(lowest > -0.01, "a tuft should not be planted underground: {lowest}");
        assert!(
            (0.2..0.6).contains(&highest),
            "and should stand about ankle high: {highest}"
        );
    }

    #[test]
    fn a_flower_carries_colour_that_is_not_a_green() {
        let mut mesh = Geometry::default();
        add(&mut mesh, Sprig::Flower, Vec3::ZERO, 0.0, 1.0, 0.5, 0.35);
        // Somewhere in it there has to be a vertex whose red beats its green,
        // which no blade of grass ever has.
        assert!(
            mesh.colours.iter().any(|c| c[0] > c[1]),
            "a flower should have a head on it"
        );
    }

    #[test]
    fn every_blade_faces_two_ways() {
        // One triangle per blade means half a meadow would be invisible from any
        // given side if it were wound once.
        let mut mesh = Geometry::default();
        add(&mut mesh, Sprig::Grass, Vec3::ZERO, 0.0, 1.0, 0.5, 0.0);
        assert_eq!(
            mesh.indices.len(),
            mesh.places.len() * 2,
            "each triangle should be wound both ways"
        );
    }

    #[test]
    fn the_scatter_keeps_clear_of_the_forest_salts() {
        // Sharing a salt with the woods would put a tuft at the foot of every
        // tree and nowhere else — the two lattices would march in step.
        let ours = [
            SALT_JITTER_X,
            SALT_JITTER_Z,
            SALT_PRESENT,
            SALT_KIND,
            SALT_TURN,
            SALT_SCALE,
            SALT_SHADE,
            SALT_PETAL,
        ];
        for salt in ours {
            assert!(salt > 7, "salt {salt} collides with the forest's");
        }
        let unique: std::collections::HashSet<u32> = ours.into_iter().collect();
        assert_eq!(unique.len(), ours.len(), "two purposes share a salt");
    }
}
