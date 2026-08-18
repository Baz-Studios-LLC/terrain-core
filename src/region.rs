//! Which part of the world you are in, and what that part of the world is like.
//!
//! # Biomes were scattered, and they should be places
//!
//! What kind of ground a point carried used to be decided entirely by that point:
//! a moisture field said dry here and wet there, and desert appeared wherever the
//! noise happened to dip. The result was correct at every point and wrong
//! everywhere — patches of desert in the middle of grassland, a stripe of wood
//! across a dune, no two hundred metres of the map the same as the next.
//!
//! That is not what a world looks like and it is not what a world is FOR. A
//! player says "the northern desert" or "the snow country in the east"; they
//! cannot say anything at all about a place whose character changes every time
//! they walk a field's width. And monsters are supposed to live in these — a
//! species that belongs to the desert needs a desert to belong to, not a scatter
//! of dry patches.
//!
//! So the map is divided into a few large regions with soft edges, and the local
//! noise is demoted to what it should always have been: variation WITHIN a place
//! rather than the thing that decides which place it is.
//!
//! # A region NAMES a country; it does not describe a climate
//!
//! This began as two physical fields — how dry, how cold — with the biome
//! inferred from them by threshold. That is how a simulation does it, and it was
//! a steady source of bugs in a game that is not one: the moisture ramp and the
//! treeline and the snowline all pushed each other about, so lowering the snow to
//! reach the coast closed the bare-rock band, and widening the desert to reach a
//! town squeezed out the grassland behind it. Every one of those was a
//! consequence nobody asked for, arrived at by arithmetic from two numbers that
//! nobody wanted to think in.
//!
//! This is a fantasy game about raising monsters. Nobody needs a humidity model
//! to say "the northern desert". So a region simply IS a country: desert, snow,
//! or the ordinary green world. The map says which, and that is the end of it.
//!
//! What is left to height and slope is what height and slope genuinely decide —
//! where the snow line sits on a mountain, which faces are too steep to hold
//! soil. And the map image, the coast and the towns still overrule everything
//! here: a desert with a river through it still has a river through it.

use glam::Vec2;

/// What kind of country somewhere is.
///
/// Three, and deliberately few. Each one is a place a player can name and a place
/// a species of monster can come from, which is the entire job.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Country {
    /// The green world: grass, woods, hills. Most of the map, and where the game
    /// begins.
    #[default]
    Ordinary,
    /// Sand and stone. Nothing grows that does not have to.
    Desert,
    /// Snow above, conifers below, and rock wherever it is too steep for either.
    Snow,
}

impl Country {
    pub fn name(self) -> &'static str {
        match self {
            Country::Ordinary => "ordinary",
            Country::Desert => "desert",
            Country::Snow => "snow",
        }
    }
}

/// # Bands, not blobs
///
/// These were ellipses, and an ellipse is the wrong shape for this job. A region
/// drawn as a blob has a rim everywhere, so it always stopped short of something:
/// the desert stopped before the north coast, the snow stopped before its own
/// shoreline, and each time the answer was to grow the blob until it covered the
/// land — which then squeezed whatever was next to it. Every one of those was the
/// same fault wearing a different number.
///
/// The map is divided by LINES instead. Each band runs from one coast to the
/// other by construction, so "the whole of this section is desert" is a thing the
/// model can express rather than a thing it has to be tuned toward.
///
/// The lines are tilted, because the continents are: a boundary that is due
/// north-south cuts a diagonal landmass at an angle nobody drew.
const TILT: f32 = 0.22;

/// Where each band begins, measured along the tilted axis, and what it is.
///
/// Read west to east. The first has no beginning — it is everything before the
/// second.
const BANDS: [(f32, Country); 4] = [
    (f32::NEG_INFINITY, Country::Ordinary),
    (0.30, Country::Desert),
    (0.63, Country::Ordinary),
    (0.79, Country::Snow),
];

/// How wide the ground between two bands is, along the same axis.
///
/// A boundary is a band of its own, not a line: sand gives way to scrub gives way
/// to grass across a walk, and the ground colour mixes the whole way. Nothing
/// about a region may be decided by which side of a line a point falls on, or the
/// line is exactly what you see.
const BLEND: f32 = 0.05;

/// How far a boundary wanders, along the same axis.
///
/// Under half the blend, so the join is still a blend with a ragged middle rather
/// than a stipple of one country in the other.
const RAGGED: f32 = 0.02;

/// A fine, repeatable 0..1 across the map, for breaking a boundary up.
///
/// Deliberately FINE. This raced a moisture field once, and any field big enough
/// to mean something about a landscape is too big to stipple with — racing it
/// only slid the boundary sideways, so the line came out wavy instead of broken.
fn speckle(uv: Vec2) -> f32 {
    let at = uv * 1_400.0;
    crate::forest::chance(at.x as i32, at.y as i32, 51)
}

/// How far along the tilted west-to-east axis a point sits.
fn along(uv: Vec2) -> f32 {
    uv.x - TILT * (uv.y - 0.5)
}

/// What country a point on the map is in, and how firmly it belongs to it.
///
/// The strength is 1 in the body of a band and falls to 0 at the boundary, which
/// is what lets the ground colour mix across the join and cover thin out toward
/// it.
pub fn at(uv: Vec2) -> (Country, f32) {
    // The boundary is broken up by nudging how far along the axis a point counts
    // as being, rather than by dithering the answer afterwards.
    //
    // One nudge, and every question asked about this point gets the same ragged
    // edge: what kind of place it is, what colour the ground is, whether a tree
    // grows here. The alternative — each caller dithering for itself — is how the
    // painter and the classifier ended up drawing two different boundaries in the
    // first place.
    let t = along(uv) + (speckle(uv) - 0.5) * RAGGED;

    // The last band that has begun by here.
    let mut index = 0;
    for (which, (from, _)) in BANDS.iter().enumerate() {
        if t >= *from {
            index = which;
        }
    }
    let country = BANDS[index].1;

    // How far from the nearer of this band's two boundaries.
    let behind = BANDS[index].0;
    let ahead = BANDS.get(index + 1).map_or(f32::INFINITY, |(from, _)| *from);
    let room = (t - behind).min(ahead - t);

    (country, crate::smoothstep(0.0, BLEND, room))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_band_reaches_both_coasts() {
        // The reason bands replaced blobs. A region has to be able to cover a
        // whole section of the map, and an ellipse never could — it has a rim
        // everywhere, so it always stopped short of something and the answer was
        // always to grow it until it squeezed its neighbour.
        //
        // Read down the map at several latitudes: every band must appear at all
        // of them, and in the same order.
        for down in 0..12 {
            let v = 0.05 + down as f32 * 0.08;
            let mut seen: Vec<Country> = Vec::new();
            for across in 0..=400 {
                let uv = Vec2::new(across as f32 / 400.0, v);
                let (country, strength) = at(uv);
                if strength < 0.6 {
                    continue;
                }
                if seen.last() != Some(&country) {
                    seen.push(country);
                }
            }
            assert_eq!(
                seen,
                vec![
                    Country::Ordinary,
                    Country::Desert,
                    Country::Ordinary,
                    Country::Snow
                ],
                "at v={v:.2} the map reads {seen:?}"
            );
        }
    }

    #[test]
    fn a_boundary_is_a_band_and_not_a_line() {
        // Nothing about a region may be decided by which side of a line a point
        // falls on, or the line is exactly what you see — which it was, twice.
        let v = 0.4;
        let mut fading = 0;
        for across in 0..=400 {
            let (_, strength) = at(Vec2::new(across as f32 / 400.0, v));
            if strength > 0.05 && strength < 0.95 {
                fading += 1;
            }
        }
        // Three boundaries, each a band of its own.
        assert!(
            fading > 40,
            "only {fading} of 400 steps across the map are in transition"
        );
    }

    #[test]
    fn the_bands_are_tilted_with_the_continents() {
        // A boundary due north-south cuts a diagonal landmass at an angle nobody
        // drew. The same boundary must sit further east in the south.
        let north = (0..400)
            .map(|n| n as f32 / 400.0)
            .find(|u| at(Vec2::new(*u, 0.1)).0 == Country::Desert)
            .expect("a desert in the north");
        let south = (0..400)
            .map(|n| n as f32 / 400.0)
            .find(|u| at(Vec2::new(*u, 0.9)).0 == Country::Desert)
            .expect("a desert in the south");
        assert!(
            south > north + 0.05,
            "the desert starts at {north:.2} in the north and {south:.2} in the south"
        );
    }

    #[test]
    fn the_world_is_mostly_ordinary_country() {
        // The exceptions are what get named, and the green world is where the
        // game starts.
        let mut ordinary = 0;
        for down in 0..60 {
            for across in 0..60 {
                let uv = Vec2::new(across as f32 / 59.0, down as f32 / 59.0);
                if at(uv).0 == Country::Ordinary {
                    ordinary += 1;
                }
            }
        }
        let share = ordinary as f32 / (60.0 * 60.0);
        assert!(
            share > 0.45,
            "the green world is only {:.0}% of the map",
            share * 100.0
        );
    }
}
