# terrain-core

The world generation Baz Studios games and **Opificium's terrain bench** both run.

## Why it exists

It used to be written twice. A game and the bench that shapes its ground both
work the world out from scratch — nothing but files pass between them — so a
difference of one digit in a hash, or two lines of a tree's shaping swapped, gave
the bench one world and the game another. No error. Nothing failing. Just wrong.

That was held together by tests pinning literal numbers copied out of one program
and asserted in the other. It worked, and it taxed every change. Written once,
the two cannot disagree at all.

This is how studios do it — an editor built *on top of* the game's runtime, with
the world code existing once. Ours are separate applications, so the shared part
is a crate rather than a module, but the principle is the same.

## It names no engine

Nothing here mentions Bevy, and it must not: the game and the bench are on
different Bevy majors and could not link the same one. It doesn't need to —
Bevy's `Vec2`/`Vec3` **are** `glam`'s, re-exported, and everything here is
arithmetic over vectors.

Geometry comes out as plain vertex arrays (`Geometry`), and each program turns
those into its own engine's mesh. That seam is a dozen lines a side and is the
only engine-shaped thing in the arrangement.

`edition = "2021"` for the same reason: the game is 2021, the bench is 2024, and
2021 is the one both can link.

## What is in it

| Module | What it decides |
| --- | --- |
| `tree` | How a tree is grown — trunk, limbs forked off it, leaves at the tips |
| `forest` | Where trees stand: the ground's own answer, plus a painted layer over it |
| root | `smoothstep`, the seeded `Draw`, and `Geometry` |

## Using it

```toml
[dependencies]
terrain-core = { git = "https://github.com/Baz-Studios-LLC/terrain-core" }
```

A **git** dependency, not a path one. The repos that use this are separate on
GitHub and their CI checks out only one, so a path dependency builds on a
developer's machine and fails every release.

## Changing it

Anything here moves ground or woods in **every world already built from it**.
`the_scatter_is_what_it_has_always_been` guards the numbers that would silently
relocate every wood in every planted world — so that stays a decision, never an
accident.

---

Baz Studios LLC
