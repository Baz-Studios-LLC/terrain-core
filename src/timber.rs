//! Building meshes out of tubes, caps and rounded lumps.
//!
//! Everything grown in this crate is made of the same handful of parts — a trunk
//! is a tube, a branch is a tube, a fallen log is a tube lying down, a boulder is
//! a lump and so is a clump of leaves. They live here rather than in whichever
//! module happened to need them first, because a second copy of the winding rule
//! below is a second chance to get it wrong.

use glam::Vec3;

use crate::{Draw, Geometry};

/// A mesh under construction. Thin wrapper over [`Geometry`], because
/// growing a tree is easier to read as `wood.tube(..)` than as index
/// arithmetic in the middle of the shaping.
#[derive(Default)]
pub(crate) struct Timber {
    pub mesh: Geometry,
    /// What to paint every vertex from here on, if anything.
    ///
    /// Trees leave it alone and wear their colour as a material, because a
    /// forest is two dozen shapes planted thousands of times and one material
    /// per variety is what makes that affordable. Anything that is one mesh
    /// wearing one material for the whole world — a boulder beside a bush beside
    /// a fallen log — has to carry its colour in its vertices instead.
    pub colour: Option<[f32; 4]>,
}

impl Timber {
    /// A tapered tube from one point to another: trunk, limb, twig.
    /// A continuous tube through a run of stations, each a place and a radius.
    ///
    /// One call per branch rather than one per segment, and — the important part
    /// — the ring's reference direction is carried FORWARD from station to
    /// station instead of derived afresh from the world axes at each one.
    ///
    /// That derivation is exactly why a trunk showed a ring at every joint. Two
    /// segments pointing only slightly differently got perpendiculars that
    /// differed a lot, because `heading.cross(Vec3::X)` swings hard for a small
    /// change in `heading` — so the two rings did not line up and the tube
    /// visibly twisted where they met. Carrying the reference along and
    /// projecting it back across each new heading keeps every ring in step.
    pub fn tube(&mut self, stations: &[(Vec3, f32)], sides: usize, cap: bool) {
        if stations.len() < 2 || sides < 3 {
            return;
        }

        let perpendicular_to = |heading: Vec3| {
            let aside = if heading.x.abs() < 0.9 { Vec3::X } else { Vec3::Z };
            heading.cross(aside).normalize()
        };

        let mut rings: Vec<u32> = Vec::with_capacity(stations.len());
        let mut reference = Vec3::X;

        for (index, &(at, radius)) in stations.iter().enumerate() {
            let heading = if index + 1 < stations.len() {
                stations[index + 1].0 - at
            } else {
                at - stations[index - 1].0
            };
            let Some(heading) = heading.try_normalize() else {
                continue;
            };

            reference = if rings.is_empty() {
                perpendicular_to(heading)
            } else {
                // The previous ring's reference, flattened back into the plane
                // across this heading. Parallel transport, and the whole trick.
                (reference - heading * reference.dot(heading))
                    .try_normalize()
                    .unwrap_or_else(|| perpendicular_to(heading))
            };
            let across = heading.cross(reference);

            rings.push(self.mesh.places.len() as u32);
            let along = index as f32 / (stations.len() - 1) as f32;
            for side in 0..sides {
                let turn = side as f32 / sides as f32 * std::f32::consts::TAU;
                let out = reference * turn.cos() + across * turn.sin();
                let place = at + out * radius;
                self.vertex(place, out, [side as f32 / sides as f32, along]);
            }
        }

        // The walls. Each quad is two triangles, and each is wound on its own
        // account — see `face`, which is where that decision lives.
        for pair in rings.windows(2) {
            let (low, high) = (pair[0], pair[1]);
            for side in 0..sides as u32 {
                let next = (side + 1) % sides as u32;
                self.face(low + side, high + side, low + next);
                self.face(low + next, high + side, high + next);
            }
        }

        if cap && rings.len() >= 2 {
            let foot = (stations[1].0 - stations[0].0).normalize_or(Vec3::Y);
            self.lid(stations[0].0, rings[0], sides, -foot);
            let last = stations.len() - 1;
            let tip = (stations[last].0 - stations[last - 1].0).normalize_or(Vec3::Y);
            self.lid(stations[last].0, rings[rings.len() - 1], sides, tip);
        }
    }

    /// One triangle, wound so that it faces the way its own corners say it does.
    ///
    /// Every triangle in a tree goes through here, and that is the point.
    ///
    /// A triangle is the unit the renderer culls, and it is culled by the order
    /// of its three corners while it is LIT by the normals stored at them. When
    /// those two disagree the triangle is shaded as though facing you and thrown
    /// away as though facing off, so you see straight through the surface. Every
    /// tube in every tree used to disagree — a trunk was a crescent of its own
    /// dark interior, limbs behind it showed through it, and it was never the
    /// material. Those are the transparent trees.
    ///
    /// Winding a whole tube at once left seven faces of a spruce still inverted.
    /// Winding each QUAD left the same seven, because a quad is two triangles and
    /// only the first one was ever asked: where a limb turns through an elbow the
    /// second half of the quad faces somewhere else, and it was handed whatever
    /// its neighbour had decided. Asking the triangle removes the last place the
    /// question can be answered on something else's behalf.
    pub fn face(&mut self, a: u32, b: u32, c: u32) {
        let outward = {
            let corner = |i: u32| Vec3::from_array(self.mesh.places[i as usize]);
            let says = |i: u32| Vec3::from_array(self.mesh.normals[i as usize]);
            let wound = (corner(b) - corner(a)).cross(corner(c) - corner(a));
            wound.dot(says(a) + says(b) + says(c)) >= 0.0
        };
        if outward {
            self.mesh.indices.extend_from_slice(&[a, b, c]);
        } else {
            self.mesh.indices.extend_from_slice(&[a, c, b]);
        }
    }

    /// A flat disc closing one end of a tube.
    pub fn lid(&mut self, at: Vec3, ring: u32, sides: usize, facing: Vec3) {
        let middle = self.mesh.places.len() as u32;
        self.vertex(at, facing, [0.5, 0.5]);

        // `facing` gives the centre its normal; the winding is the triangle's own
        // business. It used to be decided here from `facing.y <= 0.0`, which is
        // meaningless for a limb — a branch leaves the trunk near horizontal, so
        // every cap took the same arm and half came out inside-out.
        for side in 0..sides as u32 {
            let next = (side + 1) % sides as u32;
            self.face(middle, ring + side, ring + next);
        }
    }

    /// A rough ball of leaves.
    ///
    /// An octahedron pushed out at every vertex by a different amount, which at
    /// any distance a forest is seen from reads as a clump of foliage and costs
    /// six vertices. A sphere would cost twenty times that to look no better.
    pub fn blob(&mut self, at: Vec3, radius: f32, draw: &mut Draw) {
        // Rounded, not an octahedron. Six vertices and eight flat faces is a
        // SHARD however its normals are set — the shading can be made smooth but
        // the silhouette stays a diamond, and a canopy of them reads as a heap of
        // plates. That is exactly what the clouds looked like for the same
        // reason, and the fix is the same: split the faces and push them out.
        //
        // One split rather than two. There are hundreds of these on a tree where
        // there are a handful of puffs on a cloud, so the budget is spent
        // differently: thirty-two faces apiece is round enough to lose the
        // diamond, and a hundred and twenty-eight would be a hundred thousand
        // vertices of oak.
        let squash = Vec3::new(1.0, 0.78, 1.0);
        // One wobble for the whole clump, not one per vertex. Jittering vertices
        // is what made these shards in the first place; jittering whole clumps
        // is what makes a canopy irregular.
        let wobble = draw.between(0.74, 1.26);

        let (corners, faces) = crate::ball(1);
        let base = self.mesh.places.len() as u32;
        for out in corners {
            let place = at + out * radius * wobble * squash;
            self.vertex(place, out, [0.5, 0.5]);
        }
        for face in faces {
            // Through the same gate as the wood. The ball's own winding is
            // already outward, but a squashed ball is not quite the ball whose
            // normals it kept, and nothing in a tree should be the one place
            // where that is taken on trust.
            self.face(base + face[0], base + face[1], base + face[2]);
        }
    }

    /// One vertex. The only place a vertex is added, so the only place that has
    /// to remember every array must stay the same length as every other.
    fn vertex(&mut self, place: Vec3, facing: Vec3, uv: [f32; 2]) {
        self.mesh.places.push([place.x, place.y, place.z]);
        self.mesh.normals.push([facing.x, facing.y, facing.z]);
        self.mesh.uvs.push(uv);
        if let Some(colour) = self.colour {
            self.mesh.colours.push(colour);
        }
    }

    pub fn finish(self) -> Geometry {
        self.mesh
    }
}
