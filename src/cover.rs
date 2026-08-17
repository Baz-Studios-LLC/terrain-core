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
/// can ever be: a 128 m chunk holds about six thousand slots at this spacing.
/// Tightening it multiplies the vertex count by the square, so it is the first
/// number to look at if the frame rate drops.
///
/// It was 2.6, then 1.7, then 1.15, and that is most of why the ground read as
/// sprigs
/// dotted about rather than as grass. A tuft is a hand's width across; one every
/// two and a half metres is not a sward however many of them there are, because
/// the eye reads the GAPS.
///
/// What pays for closing them is that cover clumps — see [`patch`] — so almost
/// all of this is spent inside meadows and hardly any of it on the bare ground
/// between, where it would only be stubble again.
pub const SPACING: f32 = 1.0;

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

/// How strongly the cover clumps at a point: 0 on bare ground between patches,
/// 1 in the middle of a meadow.
///
/// # Grass grows in patches, and that is the whole point of this
///
/// Spread a biome's worth of grass evenly and every field in the world carries
/// the same thin stubble — which is what it looked like. Real ground is not like
/// that: grass goes where the water and the light and the soil are, so it comes
/// in meadows with thinner ground between them, and it is the PATCHES that make
/// somewhere look like a place rather than a texture.
///
/// So the same amount of grass is gathered rather than spread. A patch core is
/// solid and taller, the ground between is nearly bare, and the average over a
/// hillside is about what it was — which is why this costs nothing.
///
/// Two octaves of smooth value noise off the shared hash, at forty metres and at
/// fifteen. The big one lays out the meadows and the small one keeps their edges
/// from being circles.
///
/// Not every biome clumps, and desert is the reason this takes one at all. Dry
/// scrub is sporadic BY NATURE — that is what makes it read as dry — so gathering
/// it into lush patches would be inventing oases. It, and bare rock and snow, are
/// told to stay as they are.
pub fn patch(biome: Biome, at: Vec2) -> f32 {
    if !matches!(
        biome,
        Biome::Grass | Biome::Forest | Biome::Shore | Biome::Settled
    ) {
        return SPREAD_EVENLY;
    }

    let rough = field(at, MEADOW_WIDE, SALT_PATCH_WIDE) * 0.65
        + field(at, MEADOW_FINE, SALT_PATCH_FINE) * 0.35;
    // Squared off into actual patches. Left as it comes, the field is a gentle
    // wobble and the grass merely gets slightly thicker in places; pushed
    // through a smoothstep, it has middles and edges and gaps.
    crate::smoothstep(MEADOW_EDGE.0, MEADOW_EDGE.1, rough)
}

/// Smooth value noise over a lattice of the given size, from the shared hash.
fn field(at: Vec2, cell: f32, salt: u32) -> f32 {
    let on = at / cell;
    let low = on.floor();
    let across = on - low;
    // Eased both ways, or the patches come out as diamonds with straight sides.
    let ease = Vec2::new(
        across.x * across.x * (3.0 - 2.0 * across.x),
        across.y * across.y * (3.0 - 2.0 * across.y),
    );

    let corner = |step_x: i32, step_z: i32| {
        chance(low.x as i32 + step_x, low.y as i32 + step_z, salt)
    };
    let near = corner(0, 0) * (1.0 - ease.x) + corner(1, 0) * ease.x;
    let far = corner(0, 1) * (1.0 - ease.x) + corner(1, 1) * ease.x;
    near * (1.0 - ease.y) + far * ease.y
}

/// How big a tuft stands, given how deep in a patch it is.
///
/// Grass in the middle of a meadow is taller than grass at the edge of one,
/// because that is where the growing is good — and a patch that is only DENSER
/// than its surroundings reads as more of the same rather than as a meadow.
pub fn stature(patch: f32) -> f32 {
    STATURE.0 + (STATURE.1 - STATURE.0) * patch
}

/// How much cover a place carries, 0 to 1.
///
/// `sureness` is [`Biome::confidence`] — how strongly the ground reads as its own
/// kind. Cover fades with it rather than stopping at a boundary, so a meadow
/// thins into a wood instead of ending along a line.
///
/// `patch` is [`patch`]: the same amount of grass, gathered instead of spread.
pub fn density(biome: Biome, sureness: f32, patch: f32) -> f32 {
    thinly(biome, sureness) * (THICKNESS.0 + (THICKNESS.1 - THICKNESS.0) * patch)
}

/// What a biome would carry if it were spread evenly — the average a patch field
/// is swung either side of.
fn thinly(biome: Biome, sureness: f32) -> f32 {
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
        // Trodden rather than bare. Levelling a farmyard does not sterilise it,
        // and a hundred-and-thirty-metre disc of nothing around every town and
        // the player's own ranch is a hole in the world rather than a feature —
        // which is exactly how it looked.
        Biome::Settled => 0.14,
        // And nothing at all grows in open water.
        Biome::Water => 0.0,
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
        // Rock and snow carry the odd hardy tuft and never a flower. Nor does
        // trodden ground: a yard people cross is grass and weeds, not a meadow.
        Biome::Rock | Biome::Snow | Biome::Settled => Sprig::Grass,
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

/// How far a patch swings the thickness either side of the biome's average.
///
/// Bare between patches and well over the top inside them. Past one means every
/// slot in a core carries a tuft, so a core is SOLID rather than merely thick —
/// and the low end is deliberately almost nothing, because every tuft not spent
/// on stubble between the meadows is one that can be spent inside them.
const THICKNESS: (f32, f32) = (0.06, 1.9);

/// How many more blades a tuft grows in the middle of a patch than at its edge.
///
/// The other half of how thick a thicket is, and the half that keeps working
/// after the first one stops. Once a core is dense enough that every slot on the
/// lattice carries a tuft — which it is — the only ways left to fill it are to
/// put the slots closer together or to make each tuft fuller, and a blade added
/// to a tuft that already exists is cheaper than a whole new tuft.
const LUSH_BLADES: usize = 7;

/// How much deeper in colour a tuft is in the middle of a patch.
const LUSH_SHADE: f32 = 0.3;

/// How much taller a tuft stands in the middle of a patch than at its edge.
///
/// Three times over, which is a lot and is the point. Grass that is merely
/// thicker in places reads as a slightly better lawn; grass that comes up past
/// your knee is a different KIND of ground, and you can see from across a field
/// where it starts and stops. That is what makes it somewhere a wild monster
/// lives rather than decoration.
const STATURE: (f32, f32) = (0.7, 2.3);

/// What a biome that does not clump gets told.
const SPREAD_EVENLY: f32 = 0.5;

/// How wide the meadows are, in metres, and how wide the detail on their edges.
const MEADOW_WIDE: f32 = 40.0;
const MEADOW_FINE: f32 = 15.0;

/// The band the patch field is squared off across.
///
/// Narrow, so there are middles and gaps rather than a wobble; not so narrow that
/// a meadow has a hard rim. Centred a little above a half, which is what leaves
/// rather more open ground than thicket — a field with tall grass in it, not a
/// thicket with clearings.
///
/// Tighter than it was, because a player has to be able to SEE where the tall
/// grass begins to decide whether to walk into it.
const MEADOW_EDGE: (f32, f32) = (0.44, 0.62);

/// Salts the cover uses. Numbered clear of the forest's, which owns 1 to 7.
pub const SALT_JITTER_X: u32 = 11;
pub const SALT_JITTER_Z: u32 = 12;
pub const SALT_PRESENT: u32 = 13;
pub const SALT_KIND: u32 = 14;
pub const SALT_TURN: u32 = 15;
pub const SALT_SCALE: u32 = 16;
pub const SALT_SHADE: u32 = 17;
pub const SALT_PETAL: u32 = 18;
/// And the two the meadows are laid out with.
pub const SALT_PATCH_WIDE: u32 = 19;
pub const SALT_PATCH_FINE: u32 = 20;

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
///
/// A patch core takes this to well over a metre — see [`STATURE`]. That is
/// deliberately waist-high on a person rather than ankle-high: tall grass is
/// somewhere a monster can be without being seen, and grass you can see over is
/// scenery instead of cover.
const HEIGHT: f32 = 0.5;

/// Adds one tuft to a mesh being built.
///
/// `turn` spins it about its own base, `scale` sizes it, and `shade` and `petal`
/// are rolls for its colour. Everything is appended in place, because a chunk's
/// worth of these is one mesh and building it is one loop.
// Eight, and clippy is right to count them. They are eight because a tuft is
// decided by eight independent rolls, and bundling them into a struct would move
// the same eight one line up and add a name nobody would otherwise need.
#[allow(clippy::too_many_arguments)]
pub fn add(
    into: &mut Geometry,
    kind: Sprig,
    at: Vec3,
    turn: f32,
    scale: f32,
    shade: f32,
    petal: f32,
    lush: f32,
) {
    let (dark, light, blades) = match kind {
        Sprig::Grass => (GRASS_DARK, GRASS_LIGHT, 4),
        Sprig::Flower => (GRASS_DARK, GRASS_LIGHT, 2),
        // Splayed and stiff, and more of them: a scrub clump is what a dry place
        // has instead of a sward.
        Sprig::Scrub => (SCRUB_DARK, SCRUB_LIGHT, 5),
    };

    // Fuller the deeper into a patch it stands. Tall grass has to be THICK as
    // well as tall — a tuft of four blades stretched to a metre is a spider, not
    // a thicket — and a blade costs one triangle, so this is the cheapest kind of
    // density there is.
    //
    // Scrub is left alone. Dry country's clumps are sparse by nature and `patch`
    // never gathers them anyway.
    let blades = if kind == Sprig::Scrub {
        blades
    } else {
        blades + (LUSH_BLADES as f32 * lush).round() as usize
    };

    // And deeper in colour. A thicket is shaded by its own depth, which is what
    // makes a patch of it read as a mass rather than as more of the same grass.
    let green = shade_of(mix(dark, light, shade), 1.0 - LUSH_SHADE * lush);
    let tall = HEIGHT * scale;
    // Wider too, in step with how tall it has grown, or a metre-high blade comes
    // out as a wire.
    let wide = 0.035 * scale * (1.0 + 0.5 * lush);

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
    fn grass_gathers_into_patches_and_desert_does_not() {
        // The fault this fixes: cover spread evenly is the same thin stubble on
        // every field in the world. Gathering the SAME amount into meadows is
        // what makes somewhere look like a place.
        let mut most = 0.0_f32;
        let mut least = 1.0_f32;
        let mut total = 0.0;
        let mut looked = 0;

        // Across a few hundred metres, which is several meadows wide.
        for step_z in 0..80 {
            for step_x in 0..80 {
                let at = Vec2::new(step_x as f32 * 5.0, step_z as f32 * 5.0);
                let patch = patch(Biome::Grass, at);
                most = most.max(patch);
                least = least.min(patch);
                total += patch;
                looked += 1;
            }
        }

        assert!(most > 0.95, "no meadow ever reaches its middle: {most:.2}");
        assert!(least < 0.05, "there is no bare ground between them: {least:.2}");

        // And it has to be patches on a field, not a field with bald spots.
        let mean = total / looked as f32;
        assert!(
            (0.2..0.55).contains(&mean),
            "meadows cover {:.0}% of the ground",
            mean * 100.0
        );

        // Costing about what evenly-spread cover did, which is why it is free.
        let spread = density(Biome::Grass, 1.0, SPREAD_EVENLY);
        let gathered = density(Biome::Grass, 1.0, mean);
        assert!(
            (gathered / spread - 1.0).abs() < 0.35,
            "gathering the grass changed how much there is by {:.0}%",
            (gathered / spread - 1.0) * 100.0
        );

        // Dry country is sporadic BY NATURE — that is what makes it read as dry —
        // so it must not be gathered into oases.
        for bare in [Biome::Desert, Biome::Rock, Biome::Snow] {
            let corners: Vec<f32> = (0..40)
                .map(|step| patch(bare, Vec2::new(step as f32 * 37.0, step as f32 * 23.0)))
                .collect();
            assert!(
                corners.windows(2).all(|w| w[0] == w[1]),
                "{bare:?} is being gathered into patches"
            );
        }
    }

    #[test]
    fn a_meadow_is_solid_and_the_ground_between_is_not() {
        // A patch has to be thicker AND taller than what surrounds it. One
        // without the other reads as more of the same rather than as a meadow.
        let core = density(Biome::Grass, 1.0, 1.0);
        let gap = density(Biome::Grass, 1.0, 0.0);
        assert!(core >= 1.0, "a meadow's middle is not solid: {core:.2}");
        assert!(gap < 0.2, "the ground between meadows is not bare: {gap:.2}");
        assert!(stature(1.0) > stature(0.0) * 1.4, "a meadow is no taller");
    }

    #[test]
    fn nothing_grows_in_open_water() {
        assert_eq!(density(Biome::Water, 1.0, SPREAD_EVENLY), 0.0);
    }

    #[test]
    fn lean_ground_is_sparse_rather_than_sterile() {
        // Rock, snow and trodden yards carry a little. Nothing at all was the
        // first answer, and it made the ranch and every town a bare disc — a
        // levelled farmyard is trodden, not paved.
        for lean in [Biome::Rock, Biome::Snow, Biome::Settled] {
            let some = density(lean, 1.0, SPREAD_EVENLY);
            assert!(some > 0.0, "{} should carry something", lean.name());
            assert!(
                some < density(Biome::Grass, 1.0, SPREAD_EVENLY) * 0.4,
                "{} should carry far less than open country: {some}",
                lean.name()
            );
        }
        // And a yard grows no flowers, whatever roll it gets.
        for step in 0..64 {
            assert_eq!(kind(Biome::Settled, step as f32 / 63.0), Sprig::Grass);
        }
    }

    #[test]
    fn open_country_carries_the_most_and_a_wood_carries_less() {
        let meadow = density(Biome::Grass, 1.0, SPREAD_EVENLY);
        let under_trees = density(Biome::Forest, 1.0, SPREAD_EVENLY);
        let dry = density(Biome::Desert, 1.0, SPREAD_EVENLY);
        assert!(meadow > under_trees, "{meadow} vs {under_trees}");
        assert!(under_trees > dry, "{under_trees} vs {dry}");
    }

    #[test]
    fn cover_thins_toward_a_boundary_rather_than_stopping_at_it() {
        // A meadow should fade into a wood, not end along a line somebody can
        // see from the air.
        let sure = density(Biome::Grass, 1.0, SPREAD_EVENLY);
        let edge = density(Biome::Grass, 0.0, SPREAD_EVENLY);
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
    fn tall_grass_is_tall_enough_to_lose_something_in() {
        // The point of a thicket is that a wild monster can be in it without
        // being seen. Grass you can see over is scenery.
        let reach = |lush: f32| {
            let mut mesh = Geometry::default();
            add(
                &mut mesh,
                Sprig::Grass,
                Vec3::ZERO,
                0.0,
                stature(lush),
                0.5,
                0.0,
                lush,
            );
            (
                mesh.places.iter().map(|p| p[1]).fold(0.0_f32, f32::max),
                mesh.indices.len(),
            )
        };

        let (thin, thin_faces) = reach(0.0);
        let (thick, thick_faces) = reach(1.0);

        assert!(
            thick > 0.9,
            "the middle of a patch is only {thick:.2} m tall — you can see over it"
        );
        assert!(
            thin < 0.5,
            "the edge of a patch is {thin:.2} m tall, which is not an edge"
        );
        // And thick as well as tall. A few blades stretched to a metre is a
        // spider, not a thicket.
        assert!(
            thick_faces > thin_faces * 2,
            "a thicket tuft has {thick_faces} triangles against a thin one's {thin_faces}"
        );
    }

    #[test]
    fn a_tuft_stands_on_the_ground_and_reaches_up_from_it() {
        let mut mesh = Geometry::default();
        add(&mut mesh, Sprig::Grass, Vec3::ZERO, 0.0, 1.0, 0.5, 0.0, 1.0);

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
        add(&mut mesh, Sprig::Flower, Vec3::ZERO, 0.0, 1.0, 0.5, 0.35, 1.0);
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
        add(&mut mesh, Sprig::Grass, Vec3::ZERO, 0.0, 1.0, 0.5, 0.0, 1.0);
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
