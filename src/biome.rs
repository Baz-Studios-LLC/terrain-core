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
    /// What country this is, and how firmly it belongs to it.
    ///
    /// This was a moisture level, and the biome was inferred from it by
    /// threshold. See [`crate::region`] for why it is not any more.
    pub country: crate::region::Country,
    pub belonging: f32,
    /// How wooded ordinary country is here, 0 open to 1 dense.
    ///
    /// The one thing still decided by a noise field, and it decides only whether
    /// a patch of the green world is meadow or wood — never which country it is
    /// in. That distinction is the whole point.
    pub wooded: f32,
    /// Metres to the nearest coast, positive inland and negative offshore.
    pub shore: f32,
    /// How much a settlement has levelled this, 0 to 1.
    pub levelled: f32,
    /// Metres of standing fresh water above this ground — a river, a lake — or
    /// nought where it is dry.
    ///
    /// Separate from the height, and it has to be. The sea is anything below
    /// nought and needs nothing said about it; a river runs at forty metres up a
    /// valley and is every bit as wet. Without this a river is classified as
    /// whatever the land it cut through was, and nothing that lives in water can
    /// be told where to live.
    pub water_above: f32,
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
        use crate::region::Country;

        // Under the sea, or under a river. Both are water and neither cares what
        // the ground beneath happens to be made of.
        if ground.height < 0.0 || ground.water_above > 0.0 {
            return Biome::Water;
        }
        // Before the slope test, because a levelled town on a hillside is still a
        // town, and after the water test, because nobody levels the sea.
        if ground.levelled > sea.settled_above {
            return Biome::Settled;
        }
        let holding = crate::region::holding(ground.country, ground.belonging, ground.wooded);

        // A beach is measured from the coast rather than from its height: a
        // clifftop ten metres up is not a beach, and a sandbar is.
        //
        // Except in the cold, where there are no beaches. Snow country stopped
        // just short of its own coastline and left a ring of sand round it, which
        // is a beach nobody would ever sunbathe on; the snow runs to the water.
        if ground.shore < sea.shore_within && holding != Country::Snow {
            return Biome::Shore;
        }

        // And now the country simply says what this is.
        //
        // It used to be inferred: moisture under a threshold meant desert, over
        // another meant forest, and height over a snowline meant snow. Every one
        // of those numbers pushed the others about, and the world kept coming out
        // in ways nobody had asked for. What is left to height and slope here is
        // only what they genuinely decide — where snow sits on a mountain, and
        // which faces are too steep to hold anything.
        match holding {
            Country::Desert => {
                if ground.slope > sea.rock_above {
                    Biome::Rock
                } else {
                    Biome::Desert
                }
            }
            Country::Snow => {
                // Slope FIRST here, unlike the green world.
                //
                // A face too steep to hold soil is also too steep to hold snow,
                // and testing height first made the great mountain solid white
                // from the snowline up — nought rock samples on it, measured. A
                // white cone with no stone showing is the "giant white pimple"
                // this world has been round once already.
                if ground.slope > sea.rock_above {
                    Biome::Rock
                } else if ground.height > sea.cold_snowline {
                    Biome::Snow
                } else if ground.height > sea.cold_snowline * ROCK_BAND {
                    // Bare stone between the last conifer and the first snow.
                    //
                    // DERIVED from the snowline rather than given a number of its
                    // own, and that is the point. When these were two independent
                    // lines they walked past each other — snow beginning below
                    // where trees stopped closed the band entirely and left a
                    // mountain that went from wood straight to white. A fraction
                    // of the line above it cannot do that, whatever the line is
                    // moved to.
                    Biome::Rock
                } else {
                    // Conifers below the stone. A snow region with nothing living
                    // in it is as wrong as one wooded to the summit.
                    Biome::Forest
                }
            }
            Country::Ordinary => {
                if ground.height > sea.snowline {
                    Biome::Snow
                } else if ground.slope > sea.rock_above {
                    Biome::Rock
                } else if ground.height > sea.treeline {
                    // Above where anything grows is MOUNTAIN, not alpine meadow.
                    Biome::Rock
                } else if ground.wooded > sea.forest_above {
                    Biome::Forest
                } else {
                    Biome::Grass
                }
            }
        }
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
            // Whichever kind of water it is, and however deep. A hand's depth of
            // river is barely water; a channel is unmistakably so.
            Biome::Water => smoothstep(0.0, -2.0, ground.height)
                .max(smoothstep(0.0, 1.5, ground.water_above)),
            Biome::Shore => smoothstep(sea.shore_within, sea.shore_within * 0.3, ground.shore),
            Biome::Settled => smoothstep(sea.settled_above, 1.0, ground.levelled),
            Biome::Snow => match ground.country {
                crate::region::Country::Snow => {
                    smoothstep(sea.cold_snowline, sea.cold_snowline + 40.0, ground.height)
                }
                _ => smoothstep(sea.snowline, sea.snowline + 40.0, ground.height),
            },
            // Either reason for being rock: too steep to hold soil, or too high
            // for anything to grow. Whichever is the stronger claim answers.
            Biome::Rock => smoothstep(sea.rock_above, sea.rock_above + 0.2, ground.slope)
                .max(smoothstep(sea.treeline, sea.treeline + 50.0, ground.height)),
            // A desert is as sure of itself as the region is. That is the whole
            // gain from naming a country rather than inferring one: how firmly
            // somewhere belongs to its region is a thing the region already
            // knows, where a moisture threshold could only ever be asked how far
            // past it a number had got.
            Biome::Desert => ground.belonging,
            // Conifers in snow country are as sure as the country; a wood in the
            // green world is as sure as it is wooded.
            Biome::Forest => match ground.country {
                crate::region::Country::Snow => ground.belonging,
                _ => smoothstep(sea.forest_above, sea.forest_above + 0.18, ground.wooded),
            },
            // Grass is what is left over, so it is most itself well short of
            // becoming a wood.
            Biome::Grass => smoothstep(sea.forest_above, sea.forest_above - 0.25, ground.wooded),
        }
    }
}

/// Where the bare stone starts in snow country, as a share of its snowline.
///
/// Below this it is conifers, above it stone, and above the line itself snow.
const ROCK_BAND: f32 = 0.55;

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
    /// How wooded ordinary country has to be before it counts as a wood.
    pub forest_above: f32,
    /// The height above which SNOW country is snow, and below which it grows
    /// conifers.
    ///
    /// One number where there were two. The treeline and the snowline used to be
    /// derived separately from a coldness, and they walked past each other — snow
    /// starting below where trees stopped closed the band between them, and a
    /// mountain went from wood straight to white. In snow country the trees stop
    /// exactly where the snow starts, because it is the same line.
    pub cold_snowline: f32,
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
            forest_above: 0.52,
            cold_snowline: 45.0,
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
            country: crate::region::Country::Ordinary,
            belonging: 1.0,
            wooded: 0.45,
            shore: 800.0,
            levelled: 0.0,
            water_above: 0.0,
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
    fn the_country_decides_what_a_place_is() {
        use crate::region::Country;
        let sea = Climate::default();
        let of = |ground: Ground| Biome::of(ground, &sea);

        // This used to be `rain_decides_between_desert_grass_and_forest`, and
        // that is exactly the thing that stopped being true. A moisture level
        // under a threshold used to mean desert, which meant the desert went
        // wherever a noise field dipped and moved whenever any neighbouring
        // number was tuned. The map names the country now.
        assert_eq!(of(Ground { country: Country::Desert, ..land() }), Biome::Desert);
        assert_eq!(of(Ground { country: Country::Snow, height: 300.0, ..land() }), Biome::Snow);

        // Snow country grows conifers below its snowline rather than nothing.
        assert_eq!(of(Ground { country: Country::Snow, height: 10.0, ..land() }), Biome::Forest);

        // And in the green world, how wooded somewhere is decides meadow or wood
        // — which is the one thing a noise field is still allowed to decide, and
        // it decides it WITHIN a country rather than deciding which country.
        assert_eq!(of(Ground { wooded: 0.2, ..land() }), Biome::Grass);
        assert_eq!(of(Ground { wooded: 0.9, ..land() }), Biome::Forest);

        // A desert is a desert however wet the noise says it is.
        assert_eq!(
            of(Ground { country: Country::Desert, wooded: 1.0, ..land() }),
            Biome::Desert,
            "a noise field is overruling the map again"
        );
    }

    #[test]
    fn a_river_is_water_wherever_it_runs() {
        // The sea is anything below nought and needs nothing said about it. A
        // river runs at forty metres up a valley and is every bit as wet — and
        // until the ground could say so, a river was classified as the land it
        // cut through and nothing aquatic had anywhere to live.
        let river = Ground { water_above: 1.2, ..land() };
        assert_eq!(of(river), Biome::Water);
        assert!(
            Biome::confidence(river, &Climate::default()) > 0.5,
            "a channel's worth of water should read as water"
        );

        // Even where the land it cut would have been something else entirely.
        for ground in [
            Ground { water_above: 0.8, wooded: 0.05, ..land() },
            Ground { water_above: 0.8, height: 300.0, ..land() },
            Ground { water_above: 0.8, slope: 0.9, ..land() },
        ] {
            assert_eq!(of(ground), Biome::Water, "{ground:?}");
        }

        // And dry ground is not water, however wet the climate.
        assert_ne!(of(Ground { wooded: 1.0, ..land() }), Biome::Water);
    }

    #[test]
    fn water_wins_over_everything() {
        // Under the waterline nothing else is worth asking. A drowned forest is
        // water, and so is a sunken town.
        for ground in [
            Ground { height: -1.0, ..land() },
            Ground { height: -40.0, wooded: 1.0, ..land() },
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
            of(Ground { height: 200.0, wooded: 1.0, ..land() }),
            Biome::Rock
        );
        // And gentle ground below the treeline is still grass, however high the
        // world's hills get.
        assert_eq!(
            of(Ground { height: 140.0, wooded: 0.45, ..land() }),
            Biome::Grass
        );
    }

    #[test]
    fn confidence_falls_off_at_a_boundary_and_holds_inside_one() {
        let sea = Climate::default();
        // Well into the desert against barely into it.
        //
        // Which is now a question about the REGION rather than about a moisture
        // reading — the desert is as sure of itself as the map is that this is
        // the desert, and that is the whole gain from naming a country instead of
        // inferring one.
        let deep = Biome::confidence(
            Ground { country: crate::region::Country::Desert, belonging: 1.0, ..land() },
            &sea,
        );
        let edge = Biome::confidence(
            Ground { country: crate::region::Country::Desert, belonging: 0.12, ..land() },
            &sea,
        );
        assert!(deep > 0.9, "the deep desert should be sure of itself: {deep}");
        assert!(edge < 0.3, "its edge should not be: {edge}");

        // And every kind answers between nought and one, whatever it is asked.
        for wooded in [0.0, 0.2, 0.4, 0.6, 0.8, 1.0] {
            for country in [
                crate::region::Country::Ordinary,
                crate::region::Country::Desert,
                crate::region::Country::Snow,
            ] {
            for height in [-10.0, 5.0, 60.0, 200.0, 320.0] {
                for slope in [0.0, 0.5, 0.95] {
                    let ground = Ground { wooded, country, height, slope, ..land() };
                    let sure = Biome::confidence(ground, &sea);
                    assert!(
                        (0.0..=1.0).contains(&sure),
                        "{ground:?} answered {sure}"
                    );
                }
            }
            }
        }
    }

    #[test]
    fn every_kind_is_reachable() {
        use crate::region::Country;
        // A kind nothing can ever be is a kind that should not exist. Each one
        // here is somewhere a monster is meant to live, so an unreachable one is
        // a habitat with no ground in it.
        let sea = Climate::default();
        let found: std::collections::HashSet<Biome> = [
            Ground { height: -10.0, ..land() },
            Ground { shore: 10.0, ..land() },
            land(),
            Ground { wooded: 0.9, ..land() },
            Ground { country: Country::Desert, ..land() },
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
