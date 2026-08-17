//! What kind of place this is.
//!
//! One question, asked by everything: which trees grow here, whether there is
//! grass to draw, what colour the ground is, and — the reason it matters most —
//! which monsters live here. A world where some monsters are found in forests and
//! some in deserts needs the world itself to know which is which.
//!
//! # A kind, and separately a look
//!
//! [`Biome::of`] answers with ONE kind, because habitat is a yes or no: a monster
//! either lives in the desert or it does not. Appearance is the opposite — a
//! desert does not begin at a line, it dries out — so colour and ground cover
//! blend across the same signals rather than reading this enum. Both take their
//! thresholds from the constants here, so a boundary you can see is the boundary
//! that decides what lives there.
//!
//! # It is told, not measured
//!
//! Everything here takes the signals as arguments. This crate has no map, no
//! noise field and no world; the game and the bench each answer for their own
//! ground, and both get the same classification out. That is the whole point —
//! a monster found in a forest at the bench must be found in a forest in the
//! game.

use crate::smoothstep;

/// What the ground at a point is like, as far as deciding its kind goes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ground {
    /// Metres above the waterline. Negative is under it.
    pub height: f32,
    /// 0 flat, 1 vertical. One minus the surface normal's upward part.
    pub slope: f32,
    /// 0 parched to 1 sodden.
    pub moisture: f32,
    /// Metres to the nearest coast, positive inland and negative offshore.
    pub shore: f32,
    /// How much a settlement has levelled this, 0 to 1.
    pub levelled: f32,
}

/// The kinds of place a world has.
///
/// Eight, and each has to earn its place twice: it must look different from its
/// neighbours AND be somewhere different things live. A kind that only looked
/// different would belong in the colouring, not here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Biome {
    /// Sea, lake or river. Where anything that swims is found.
    Water,
    /// Sand and shingle at the waterline.
    Shore,
    /// Open country: meadow, plain, pasture. The default of a temperate world.
    Grass,
    /// Wooded ground, wherever the trees closed over.
    Forest,
    /// Dry country — parched, and low enough to be hot.
    Desert,
    /// Bare stone: cliffs, scree, anything too steep to hold soil.
    Rock,
    /// The tops, above where anything grows.
    Snow,
    /// Ground somebody levelled to build on. Not wild, and nothing wild lives
    /// here — which is exactly why it is worth being able to ask.
    Settled,
}

impl Biome {
    /// The name to show a person.
    pub fn name(self) -> &'static str {
        match self {
            Biome::Water => "Water",
            Biome::Shore => "Shore",
            Biome::Grass => "Grassland",
            Biome::Forest => "Forest",
            Biome::Desert => "Desert",
            Biome::Rock => "Rock",
            Biome::Snow => "Snow",
            Biome::Settled => "Settled",
        }
    }

    /// Whether wild things live here at all.
    ///
    /// Asked before anything is put in a place: open water needs swimmers and
    /// nothing else, bare rock and snow are lean, and levelled town ground has
    /// people on it.
    pub fn is_wild(self) -> bool {
        !matches!(self, Biome::Settled)
    }

    /// Every kind, for a caller that needs to iterate them — a spawn table, a
    /// legend, a debug overlay.
    pub const ALL: [Biome; 8] = [
        Biome::Water,
        Biome::Shore,
        Biome::Grass,
        Biome::Forest,
        Biome::Desert,
        Biome::Rock,
        Biome::Snow,
        Biome::Settled,
    ];

    /// What kind of place this ground is.
    ///
    /// The order of these questions IS the rule, and it runs from the most
    /// physical to the most negotiable. Under the water nothing else matters;
    /// a cliff is a cliff however wet it is; and only once a place has been
    /// found to be ordinary standable land does how much rain it gets decide
    /// between desert, grass and forest.
    pub fn of(ground: Ground, sea: &Climate) -> Self {
        if ground.height < 0.0 {
            return Biome::Water;
        }
        // Before the slope test, because a levelled town on a hillside is still a
        // town, and after the water test, because nobody levels the sea.
        if ground.levelled > sea.settled_above {
            return Biome::Settled;
        }
        // A beach is measured from the coast rather than from its height: a
        // clifftop ten metres up is not a beach, and a sandbar is.
        if ground.shore < sea.shore_within {
            return Biome::Shore;
        }
        if ground.height > sea.snowline {
            return Biome::Snow;
        }
        // Too steep to hold soil. Tested after snow so a high cliff reads as the
        // tops rather than as rock poking through them.
        if ground.slope > sea.rock_above {
            return Biome::Rock;
        }
        // Above where anything grows is MOUNTAIN, not alpine meadow. It read as
        // grass, and the result was a world with no rock in it at all — nought
        // per cent, measured — so anything meant to live in the mountains had
        // nowhere to be. A flank below the snowline is the mountain.
        if ground.height > sea.treeline {
            return Biome::Rock;
        }
        if ground.moisture < sea.desert_below {
            return Biome::Desert;
        }
        if ground.moisture > sea.forest_above {
            return Biome::Forest;
        }
        Biome::Grass
    }

    /// How strongly this ground reads as its kind, 0 at a boundary to 1 well
    /// inside one.
    ///
    /// For anything that should fade rather than switch — how thick the grass is,
    /// how often a desert monster turns up. A hard edge is right for asking WHAT
    /// a place is and wrong for everything downstream of the answer.
    pub fn confidence(ground: Ground, sea: &Climate) -> f32 {
        let kind = Biome::of(ground, sea);
        match kind {
            Biome::Water => smoothstep(0.0, -2.0, ground.height),
            Biome::Shore => smoothstep(sea.shore_within, sea.shore_within * 0.3, ground.shore),
            Biome::Settled => smoothstep(sea.settled_above, 1.0, ground.levelled),
            Biome::Snow => smoothstep(sea.snowline, sea.snowline + 40.0, ground.height),
            // Either reason for being rock: too steep to hold soil, or too high
            // for anything to grow. Whichever is the stronger claim answers.
            Biome::Rock => smoothstep(sea.rock_above, sea.rock_above + 0.2, ground.slope)
                .max(smoothstep(sea.treeline, sea.treeline + 50.0, ground.height)),
            Biome::Desert => smoothstep(sea.desert_below, sea.desert_below * 0.4, ground.moisture),
            Biome::Forest => smoothstep(sea.forest_above, sea.forest_above + 0.18, ground.moisture),
            // Grass is what is left over, so it is most itself in the middle of
            // the band rather than at one end of it.
            Biome::Grass => {
                let dry = smoothstep(sea.desert_below, sea.desert_below + 0.1, ground.moisture);
                let wet = smoothstep(sea.forest_above, sea.forest_above - 0.1, ground.moisture);
                dry.min(wet)
            }
        }
    }
}

/// Where one kind of place gives way to the next.
///
/// A world's own, not a constant, because these are the numbers a maker tunes to
/// decide what sort of world it is: the same generation with the desert threshold
/// moved is a wetter continent. They travel in `world.json` with everything else
/// that shapes the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Climate {
    /// Metres from the coast within which ground is beach.
    pub shore_within: f32,
    /// Metres above the waterline where trees stop, and where snow starts.
    pub treeline: f32,
    pub snowline: f32,
    /// Slope past which ground is bare stone.
    pub rock_above: f32,
    /// Moisture below which land is desert, and above which it is wooded.
    pub desert_below: f32,
    pub forest_above: f32,
    /// How much levelling makes ground somebody's rather than nobody's.
    pub settled_above: f32,
}

impl Default for Climate {
    /// A temperate world with dry country in it — which is what Ranger is.
    ///
    /// Tuned so the ordinary case is grass, forest is common where it rains, and
    /// desert is a region you travel to rather than a texture over everything.
    fn default() -> Self {
        Self {
            shore_within: 25.0,
            treeline: 150.0,
            snowline: 250.0,
            rock_above: 0.62,
            desert_below: 0.3,
            forest_above: 0.52,
            settled_above: 0.35,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ordinary standable land, for a test to vary one thing about.
    fn land() -> Ground {
        Ground {
            height: 40.0,
            slope: 0.1,
            moisture: 0.45,
            shore: 800.0,
            levelled: 0.0,
        }
    }

    fn of(ground: Ground) -> Biome {
        Biome::of(ground, &Climate::default())
    }

    #[test]
    fn ordinary_land_is_grass() {
        // The default of a temperate world, and what everything else is measured
        // as a departure from.
        assert_eq!(of(land()), Biome::Grass);
    }

    #[test]
    fn rain_decides_between_desert_grass_and_forest() {
        assert_eq!(of(Ground { moisture: 0.05, ..land() }), Biome::Desert);
        assert_eq!(of(Ground { moisture: 0.45, ..land() }), Biome::Grass);
        assert_eq!(of(Ground { moisture: 0.9, ..land() }), Biome::Forest);
    }

    #[test]
    fn water_wins_over_everything() {
        // Under the waterline nothing else is worth asking. A drowned forest is
        // water, and so is a sunken town.
        for ground in [
            Ground { height: -1.0, ..land() },
            Ground { height: -40.0, moisture: 1.0, ..land() },
            Ground { height: -5.0, levelled: 1.0, ..land() },
            Ground { height: -5.0, slope: 0.9, ..land() },
        ] {
            assert_eq!(of(ground), Biome::Water, "{ground:?}");
        }
    }

    #[test]
    fn a_beach_is_measured_from_the_coast_and_not_from_its_height() {
        // A clifftop ten metres up is not a beach; a sandbar is. Height cannot
        // tell those apart and distance to the water can.
        assert_eq!(of(Ground { shore: 10.0, ..land() }), Biome::Shore);
        assert_eq!(
            of(Ground { shore: 10.0, height: 30.0, ..land() }),
            Biome::Shore,
            "still within the shore band"
        );
        assert_eq!(
            of(Ground { shore: 400.0, height: 2.0, ..land() }),
            Biome::Grass,
            "low ground far inland is not a beach"
        );
    }

    #[test]
    fn a_town_is_a_town_even_on_a_hillside() {
        // Asked before the slope, because levelled ground on a slope is what a
        // town on a hillside IS, and calling it rock would put wild things in the
        // middle of a settlement.
        assert_eq!(
            of(Ground { levelled: 1.0, slope: 0.8, ..land() }),
            Biome::Settled
        );
        assert!(!Biome::Settled.is_wild(), "nothing wild lives in a town");
    }

    #[test]
    fn the_tops_are_snow_and_the_steep_is_rock() {
        assert_eq!(of(Ground { height: 300.0, ..land() }), Biome::Snow);
        assert_eq!(of(Ground { slope: 0.9, ..land() }), Biome::Rock);
        // A cliff up in the snow reads as the tops, not as rock poking through
        // them — which is why snow is asked first.
        assert_eq!(
            of(Ground { height: 300.0, slope: 0.9, ..land() }),
            Biome::Snow
        );
    }

    #[test]
    fn above_the_treeline_the_wettest_ground_is_mountain() {
        // A wood cannot grow where trees do not, whatever the rain does — and
        // what is left is MOUNTAIN, not meadow. Reading it as grass gave a world
        // with no rock in it anywhere, so anything meant to live in the mountains
        // had nowhere to be.
        assert_eq!(
            of(Ground { height: 200.0, moisture: 1.0, ..land() }),
            Biome::Rock
        );
        // And gentle ground below the treeline is still grass, however high the
        // world's hills get.
        assert_eq!(
            of(Ground { height: 140.0, moisture: 0.45, ..land() }),
            Biome::Grass
        );
    }

    #[test]
    fn confidence_falls_off_at_a_boundary_and_holds_inside_one() {
        let sea = Climate::default();
        // Well into the desert against barely into it.
        let deep = Biome::confidence(Ground { moisture: 0.05, ..land() }, &sea);
        let edge = Biome::confidence(Ground { moisture: 0.29, ..land() }, &sea);
        assert!(deep > 0.9, "the deep desert should be sure of itself: {deep}");
        assert!(edge < 0.3, "its edge should not be: {edge}");

        // And every kind answers between nought and one, whatever it is asked.
        for moisture in [0.0, 0.2, 0.4, 0.6, 0.8, 1.0] {
            for height in [-10.0, 5.0, 60.0, 200.0, 320.0] {
                for slope in [0.0, 0.5, 0.95] {
                    let ground = Ground { moisture, height, slope, ..land() };
                    let sure = Biome::confidence(ground, &sea);
                    assert!(
                        (0.0..=1.0).contains(&sure),
                        "{ground:?} answered {sure}"
                    );
                }
            }
        }
    }

    #[test]
    fn every_kind_is_reachable() {
        // A kind nothing can ever be is a kind that should not exist. Each one
        // here is somewhere a monster is meant to live, so an unreachable one is
        // a habitat with no ground in it.
        let sea = Climate::default();
        let found: std::collections::HashSet<Biome> = [
            Ground { height: -10.0, ..land() },
            Ground { shore: 10.0, ..land() },
            land(),
            Ground { moisture: 0.9, ..land() },
            Ground { moisture: 0.05, ..land() },
            Ground { height: 200.0, ..land() },
            Ground { height: 320.0, ..land() },
            Ground { levelled: 1.0, ..land() },
        ]
        .into_iter()
        .map(|ground| Biome::of(ground, &sea))
        .collect();

        for kind in Biome::ALL {
            assert!(found.contains(&kind), "{} is unreachable", kind.name());
        }
    }
}
