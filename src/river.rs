//! Rivers, found rather than drawn.
//!
//! Nobody places a river. Water falls on every part of a continent, runs
//! downhill, and gathers — and where enough of it has gathered, that is a river.
//! So this asks the ground where the water would go and believes the answer,
//! which is why the rivers come out in valleys, join as they descend, and reach
//! the sea instead of stopping in a field.
//!
//! # How
//!
//! Four steps, and each is the standard one:
//!
//! 1. **Sample** the ground onto a grid.
//! 2. **Fill the hollows.** Every pit is filled to the level of its lowest lip,
//!    so that from anywhere on the map there is a downhill path to the sea. Real
//!    terrain has pits everywhere and a generated one has thousands; without
//!    this, water gathers in each and no river ever forms.
//! 3. **Follow the water down.** Each cell drains to its lowest neighbour, and
//!    every cell's catchment is passed downstream — worked out from the top of
//!    the map down, so a cell's own total is complete before it is handed on.
//! 4. **Cut the channels.** Where the catchment passes a threshold there is a
//!    river; how much it has gathered says how wide and deep.
//!
//! # It is told the ground, and hands back a correction
//!
//! Like everything here, this crate has no world of its own. It is given a
//! function that answers how high the ground is, and hands back two grids: how
//! far to LOWER the ground at each point, and what height the water sits at.
//! Adding those to a heightfield is the whole of what a game does with it.

use glam::Vec2;
use std::collections::BinaryHeap;

/// How much of a world a channel must drain before it counts as a river.
///
/// A share of the whole map's area, and both halves of that matter.
///
/// AREA, because counting cells is not resolution-independent however it is
/// described — halve the spacing and the same valley drains four times the cells,
/// so a fixed count silently turns every creek into a river when the grid gets
/// finer.
///
/// A SHARE, because this world is eight kilometres by four. A real river wants a
/// catchment of hundreds of square kilometres and there are thirty-three here, so
/// a figure taken from life gives a continent with no water on it. What matters
/// is that the biggest valleys carry rivers and the small creases do not.
pub const RIVER_FROM: f32 = 0.004;

/// The widest a river gets, in metres, and the narrowest a channel is cut.
pub const WIDEST: f32 = 46.0;
pub const NARROWEST: f32 = 7.0;

/// How deep a channel is cut, as a fraction of how wide it is.
///
/// Rivers are far wider than they are deep — a channel cut as deep as it is wide
/// is a slot, and reads as one.
pub const DEEPNESS: f32 = 0.17;

/// How far the banks reach past the channel, as a multiple of its width.
///
/// The lesson the roads taught: a cut that resolves over a fixed distance is a
/// trench whatever else is right about it. Banks are proportional, so a big river
/// sits in a broad valley and a creek in a crease.
pub const BANKS: f32 = 3.2;

/// What the water did to a stretch of ground.
///
/// Two grids over the world: how far to lower the ground, and the height the
/// water sits at. Off the rivers the first is nought and the second is the
/// ground's own height, so both can be read between cells without a special case
/// at the bank.
pub struct Rivers {
    wide: usize,
    deep: usize,
    half: Vec2,
    /// Metres to take off the ground here.
    cut: Vec<f32>,
    /// Where the water surface sits. Meaningful where `cut` is above nothing.
    water: Vec<f32>,
    /// How many cells ended up carrying a river, for anyone who wants to know
    /// whether the thresholds are sane before looking at a map.
    channels: usize,
}

impl Rivers {
    /// No rivers at all: what a world without water asks for, and what a failed
    /// read falls back to.
    pub fn none(half: Vec2) -> Self {
        Self {
            wide: 1,
            deep: 1,
            half,
            cut: vec![0.0],
            water: vec![0.0],
            channels: 0,
        }
    }

    pub fn channel_cells(&self) -> usize {
        self.channels
    }

    /// Works the rivers out for a world.
    ///
    /// `ground` answers how high the land is before any water touched it, and
    /// must not itself consult the rivers — this is the same rule the towns
    /// follow, and for the same reason: a generator that reads its own output
    /// has no defined answer.
    pub fn carve(half: Vec2, spacing: f32, sea: f32, ground: &dyn Fn(Vec2) -> f32) -> Self {
        let wide = ((half.x * 2.0 / spacing).ceil() as usize + 1).max(2);
        let deep = ((half.y * 2.0 / spacing).ceil() as usize + 1).max(2);

        let at = |x: usize, z: usize| {
            Vec2::new(
                x as f32 * spacing - half.x,
                z as f32 * spacing - half.y,
            )
        };

        // 1. The ground, as it stands.
        let mut height = vec![0.0_f32; wide * deep];
        for z in 0..deep {
            for x in 0..wide {
                height[z * wide + x] = ground(at(x, z));
            }
        }

        // 2. Fill the hollows, so every cell can reach the sea downhill.
        let filled = fill_hollows(&height, wide, deep, sea);

        // 3. Follow the water down and total up what each cell drains.
        let (downhill, flow) = gather(&filled, wide, deep, sea, spacing * spacing);
        // What counts as a river here: a share of this world, not a figure from
        // life. See `RIVER_FROM`.
        let enough = half.x * 2.0 * half.y * 2.0 * RIVER_FROM;

        // 4. Cut the channels.
        let mut cut = vec![0.0_f32; wide * deep];
        let mut water: Vec<f32> = height.clone();
        let mut channels = 0;

        // Downstream order, so a river's surface can be held to never climb.
        let mut order: Vec<usize> = (0..wide * deep).collect();
        order.sort_by(|a, b| filled[*b].total_cmp(&filled[*a]));

        let mut surface = filled.clone();
        for &cell in &order {
            if flow[cell] < enough {
                continue;
            }
            channels += 1;

            // Wider the more it drains, but by the fourth root: a river with a
            // hundred times the catchment is about three times the river, which
            // is roughly how it works and, more to the point, stops the trunk
            // becoming a lake while the creeks stay invisible.
            let size = (flow[cell] / enough).powf(0.25);
            let width = (NARROWEST * size).min(WIDEST);
            let depth = width * DEEPNESS;

            // Where the water stands: most of the way up its own channel.
            //
            // NOT the downstream neighbour's ground, which is what this was. The
            // ground falls between one cell and the next by more than a small
            // channel is deep, so taking the level from downstream put the
            // surface BELOW the bed that had just been cut — and a river that is
            // under its own bed does not get drawn. Every inland channel came out
            // dry and only the ones at the coast, held up by the sea, had water
            // in them.
            //
            // Filled to three quarters, so a channel reads as a river with banks
            // rather than as a canal brimming over.
            let held = (surface[cell] - depth * 0.25).max(sea);

            // Stamped into the grids with banks either side, so the correction
            // is smooth where it is read between cells.
            let reach = width * BANKS;
            let span = (reach / spacing).ceil() as isize;
            let (cx, cz) = ((cell % wide) as isize, (cell / wide) as isize);
            for step_z in -span..=span {
                for step_x in -span..=span {
                    let (nx, nz) = (cx + step_x, cz + step_z);
                    if nx < 0 || nz < 0 || nx >= wide as isize || nz >= deep as isize {
                        continue;
                    }
                    let away = Vec2::new(step_x as f32, step_z as f32).length() * spacing;
                    if away > reach {
                        continue;
                    }
                    // Flat across the channel, then easing up the banks — the
                    // profile a river actually leaves.
                    let bite = depth * crate::smoothstep(reach, width * 0.5, away);
                    let index = (nz * wide as isize + nx) as usize;
                    if bite > cut[index] {
                        cut[index] = bite;
                        water[index] = held;
                    }
                }
            }
        }

        Self {
            wide,
            deep,
            half,
            cut,
            water,
            channels,
        }
    }

    /// How far the ground drops here, and what height the water sits at.
    ///
    /// Read between cells, so a bank is a slope rather than a staircase. Off the
    /// rivers the drop is nought and the height is meaningless — callers should
    /// test the drop.
    pub fn at(&self, x: f32, z: f32) -> (f32, f32) {
        let fx = (x + self.half.x) / self.spacing_x();
        let fz = (z + self.half.y) / self.spacing_z();
        if fx < 0.0 || fz < 0.0 || fx > (self.wide - 1) as f32 || fz > (self.deep - 1) as f32 {
            return (0.0, 0.0);
        }

        let x0 = fx.floor() as usize;
        let z0 = fz.floor() as usize;
        let x1 = (x0 + 1).min(self.wide - 1);
        let z1 = (z0 + 1).min(self.deep - 1);
        let tx = fx - x0 as f32;
        let tz = fz - z0 as f32;

        let blend = |grid: &[f32]| {
            let near = grid[z0 * self.wide + x0] * (1.0 - tx) + grid[z0 * self.wide + x1] * tx;
            let far = grid[z1 * self.wide + x0] * (1.0 - tx) + grid[z1 * self.wide + x1] * tx;
            near * (1.0 - tz) + far * tz
        };
        (blend(&self.cut), blend(&self.water))
    }

    fn spacing_x(&self) -> f32 {
        self.half.x * 2.0 / (self.wide - 1).max(1) as f32
    }

    fn spacing_z(&self) -> f32 {
        self.half.y * 2.0 / (self.deep - 1).max(1) as f32
    }
}

/// Fills every pit to the level of its lowest lip.
///
/// Priority flood, working inward from the sea and the map's edge: the lowest
/// unvisited cell on the frontier is always the next one settled, and a cell can
/// never settle lower than the frontier that reached it. What comes out has no
/// hollow left in it, so from anywhere there is a path downhill to the sea.
///
/// Without this there are no rivers at all — a generated heightfield has pits in
/// every hectare, and water gathers in each one and stops.
fn fill_hollows(height: &[f32], wide: usize, deep: usize, sea: f32) -> Vec<f32> {
    /// A cell on the frontier, ordered so the LOWEST comes off first.
    struct Lip(f32, usize);
    impl PartialEq for Lip {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }
    impl Eq for Lip {}
    impl PartialOrd for Lip {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }
    impl Ord for Lip {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            // Reversed, because BinaryHeap is a max-heap and this wants the least.
            other.0.total_cmp(&self.0)
        }
    }

    let mut filled = vec![f32::MAX; wide * deep];
    let mut frontier = BinaryHeap::new();

    // Everything already at or under the sea is settled, and so is the map's
    // rim — water leaving the edge of the world has left.
    for z in 0..deep {
        for x in 0..wide {
            let cell = z * wide + x;
            let edge = x == 0 || z == 0 || x == wide - 1 || z == deep - 1;
            if edge || height[cell] <= sea {
                filled[cell] = height[cell];
                frontier.push(Lip(height[cell], cell));
            }
        }
    }

    while let Some(Lip(level, cell)) = frontier.pop() {
        // A stale entry: this cell was settled lower by another path.
        if level > filled[cell] {
            continue;
        }
        for next in neighbours(cell, wide, deep) {
            if filled[next] < f32::MAX {
                continue;
            }
            // Either its own height, or the level it had to be flooded to for
            // the water to get here at all.
            filled[next] = height[next].max(level);
            frontier.push(Lip(filled[next], next));
        }
    }

    // Anything the frontier never reached keeps its own height.
    for cell in 0..filled.len() {
        if filled[cell] == f32::MAX {
            filled[cell] = height[cell];
        }
    }
    filled
}

/// Works out where each cell drains and how much it drains.
///
/// Returns the downhill neighbour of every cell and the catchment that passes
/// through it, counted in cells. Worked from the top of the map downward, so
/// every cell's own total is finished before it is handed on — which is what
/// makes one pass enough.
fn gather(
    filled: &[f32],
    wide: usize,
    deep: usize,
    sea: f32,
    cell_area: f32,
) -> (Vec<Option<usize>>, Vec<f32>) {
    let mut downhill: Vec<Option<usize>> = vec![None; filled.len()];
    for cell in 0..filled.len() {
        if filled[cell] <= sea {
            continue;
        }
        let mut best = None;
        let mut lowest = filled[cell];
        for next in neighbours(cell, wide, deep) {
            if filled[next] < lowest {
                lowest = filled[next];
                best = Some(next);
            }
        }
        downhill[cell] = best;
    }

    // Every cell starts with the rain that fell on its own ground and passes on
    // whatever reaches it. In square metres, so the answer does not change when
    // the grid does.
    let mut flow = vec![cell_area; filled.len()];
    let mut order: Vec<usize> = (0..filled.len()).collect();
    order.sort_by(|a, b| filled[*b].total_cmp(&filled[*a]));

    for &cell in &order {
        if let Some(next) = downhill[cell] {
            flow[next] += flow[cell];
        }
    }
    (downhill, flow)
}

/// The eight cells around one, kept on the grid.
fn neighbours(cell: usize, wide: usize, deep: usize) -> impl Iterator<Item = usize> {
    let (x, z) = ((cell % wide) as isize, (cell / wide) as isize);
    let (wide, deep) = (wide as isize, deep as isize);
    [
        (-1, -1), (0, -1), (1, -1),
        (-1, 0),           (1, 0),
        (-1, 1),  (0, 1),  (1, 1),
    ]
    .into_iter()
    .filter_map(move |(dx, dz)| {
        let (nx, nz) = (x + dx, z + dz);
        (nx >= 0 && nz >= 0 && nx < wide && nz < deep).then(|| (nz * wide + nx) as usize)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const HALF: Vec2 = Vec2::new(400.0, 400.0);

    /// A valley running east, with sides that fall into it.
    ///
    /// NOT a cone, and the difference is the whole point. On a cone every drop
    /// runs radially outward and the flow lines DIVERGE, so nothing ever gathers
    /// and no river can form however low the threshold is set — which is exactly
    /// what the first version of this test proved, and it proved it about the
    /// test rather than about the code.
    fn valley(at: Vec2) -> f32 {
        60.0 - (at.x + 400.0) * 0.06 + at.y.abs() * 0.12
    }

    /// The same, with a pit gouged out of the valley floor. Without hollow
    /// filling the water stops in it.
    fn valley_with_a_pit(at: Vec2) -> f32 {
        let base = valley(at);
        let pit = (at - Vec2::new(0.0, 0.0)).length();
        if pit < 45.0 {
            base - 30.0 * (1.0 - pit / 45.0)
        } else {
            base
        }
    }

    #[test]
    fn water_gathers_in_a_valley_and_cuts_it() {
        let rivers = Rivers::carve(HALF, 8.0, 0.0, &valley);
        assert!(
            rivers.channel_cells() > 0,
            "a valley this size should gather enough water to cut a channel"
        );
    }

    #[test]
    fn a_pit_does_not_stop_the_water() {
        // The whole reason the hollows are filled. A generated heightfield has
        // pits everywhere; if water stops in each one, no river ever forms and
        // the map comes out dry with no error to say why.
        let flat = Rivers::carve(HALF, 8.0, 0.0, &valley);
        let pitted = Rivers::carve(HALF, 8.0, 0.0, &valley_with_a_pit);
        assert!(
            pitted.channel_cells() > flat.channel_cells() / 2,
            "a pit swallowed the drainage: {} channels against {}",
            pitted.channel_cells(),
            flat.channel_cells()
        );
    }

    #[test]
    fn a_channel_is_cut_below_the_ground_around_it() {
        let rivers = Rivers::carve(HALF, 8.0, 0.0, &valley);

        // Somewhere on the map the ground has been taken down.
        let mut deepest = 0.0_f32;
        for step in -50..50 {
            for other in -50..50 {
                let (cut, _) = rivers.at(step as f32 * 8.0, other as f32 * 8.0);
                deepest = deepest.max(cut);
            }
        }
        assert!(deepest > 0.5, "nothing was cut anywhere: {deepest:.2} m");
        assert!(
            deepest < WIDEST * DEEPNESS * 1.1,
            "cut deeper than the widest river should be: {deepest:.2} m"
        );
    }

    #[test]
    fn banks_are_a_slope_and_not_a_step() {
        // The lesson the roads taught, and the reason the banks are proportional
        // to the width: a cut that resolves over a fixed distance is a trench.
        let rivers = Rivers::carve(HALF, 8.0, 0.0, &valley);

        let mut steepest = 0.0_f32;
        let step = 4.0;
        for z in -80..80 {
            for x in -80..80 {
                let here = rivers.at(x as f32 * step, z as f32 * step).0;
                let there = rivers.at((x + 1) as f32 * step, z as f32 * step).0;
                steepest = steepest.max((here - there).abs() / step);
            }
        }
        assert!(
            steepest < 0.6,
            "a bank climbs at {steepest:.2}, which is a wall"
        );
    }

    #[test]
    fn a_world_with_no_water_asks_for_none() {
        let rivers = Rivers::none(HALF);
        assert_eq!(rivers.channel_cells(), 0);
        assert_eq!(rivers.at(0.0, 0.0), (0.0, 0.0));
        assert_eq!(rivers.at(9_999.0, -9_999.0), (0.0, 0.0), "and off the map");
    }

    #[test]
    fn water_never_runs_uphill() {
        // The one thing everybody spots. A channel cut through a rise leaves the
        // surface climbing out of it unless the surface is held down as it goes.
        let rivers = Rivers::carve(HALF, 8.0, 0.0, &valley);
        for z in -40..40 {
            for x in -40..40 {
                let (cut, water) = rivers.at(x as f32 * 8.0, z as f32 * 8.0);
                if cut <= 0.01 {
                    continue;
                }
                let ground = valley(Vec2::new(x as f32 * 8.0, z as f32 * 8.0));
                assert!(
                    water <= ground + 0.5,
                    "water stands {:.1} m above the ground it cut",
                    water - ground
                );
            }
        }
    }
}
