//! The spray thrown up where the water breaks on the shore, which the `water` slice finds
//! (`water::Surf`): where a crest of the swell comes in, more where the bank is steep, as a
//! beach only washes and a bank of rock throws water up; and where a truck's ripple comes
//! to the shore, more for a stronger ring.
//!
//! The droplets are the spray's (`spray`): drops, and now and then a puff of mist.

use bevy::prelude::*;

use super::pool::{self, Particles};
use super::spray::{Drops, Kind, Mist, Random, throw_droplet};
use crate::track::Track;
use crate::water::{Surf, SurfKind};

/// How many droplets a crest throws at one point of the shore, which are a few metres
/// apart: on a flat beach, and on a bank as steep as `STEEP` or steeper.
const SURF_DROPLETS: [f32; 2] = [1.0, 4.0];
/// How steep a bank is steep, in metres of rise for each metre across: 45 degrees.
const STEEP: f32 = 1.0;
/// How many droplets a ring throws where it comes to the shore, for each metre high it is
/// there, and the most at one point.
const RING_DROPLETS_PER_METRE: f32 = 150.0;
const MOST_RING_DROPLETS: f32 = 5.0;
/// How many droplets the shore leaves for the trucks: it throws none while fewer than this
/// many could still be thrown.
const LEFT_FOR_TRUCKS: usize = 600;
/// How big the shore's droplets are, in metres: the smallest and the largest radius.
const SHORE_SPRAY_SIZE: [f32; 2] = [0.06, 0.16];
/// How often water thrown up at the shore brings a puff of mist with it, from 0 to 1, and
/// how big the puff is, in metres of radius.
const SHORE_MIST: f32 = 0.5;
const SHORE_MIST_SIZE: [f32; 2] = [0.3, 0.6];

/// Throws spray where the water breaks on the shore.
pub(super) fn throw_surf(
    time: Res<Time>,
    track: Res<Track>,
    mut surf: MessageReader<Surf>,
    mut drops: Query<&mut Particles, (With<Drops>, Without<Mist>)>,
    mut mist: Query<&mut Particles, (With<Mist>, Without<Drops>)>,
    mut random: Local<Random>,
) {
    let Some(level) = track.water_level else {
        return;
    };
    let (Ok(mut drops), Ok(mut mist)) = (drops.single_mut(), mist.single_mut()) else {
        return;
    };
    let clock = pool::clock(&time);
    let floor = |at: Vec2| level.max(track.heights.height_at(at.x, at.y));
    let mut room = drops.room(clock).saturating_sub(LEFT_FOR_TRUCKS);
    for surf in surf.read() {
        let count = match surf.kind {
            SurfKind::Swell => surf_droplets(surf.steepness),
            SurfKind::Ripple { height } => ring_droplets(height),
        };
        // Anywhere along the width it breaks over.
        let along = surf.inwards.perp() * random.between(-0.5, 0.5) * surf.width;
        let at = surf.at + Vec3::new(along.x, 0.0, along.y);
        // A share of a droplet is thrown as often as that share says.
        let whole = (count + random.between(0.0, 1.0)) as usize;
        for _ in 0..whole.min(room) {
            let velocity = shore_spray_velocity(&mut random, surf.inwards, surf.steepness);
            let size = random.between(SHORE_SPRAY_SIZE[0], SHORE_SPRAY_SIZE[1]);
            throw_droplet(&mut drops, Kind::Drop, clock, floor, at, velocity, size);
        }
        room = room.saturating_sub(whole);
        // And a puff of mist, where water is thrown up at all.
        if whole > 0 && room > 0 && random.next() < SHORE_MIST {
            let velocity = shore_spray_velocity(&mut random, surf.inwards, surf.steepness) * 0.5;
            let size = random.between(SHORE_MIST_SIZE[0], SHORE_MIST_SIZE[1]);
            throw_droplet(&mut mist, Kind::Mist, clock, floor, at, velocity, size);
        }
    }
}

/// How many droplets a crest throws at a point of the shore as steep as `steepness`.
fn surf_droplets(steepness: f32) -> f32 {
    let [flat, steep] = SURF_DROPLETS;
    flat + (steep - flat) * (steepness / STEEP).min(1.0)
}

/// How many droplets a ring `height` metres high throws where it comes to the shore.
fn ring_droplets(height: f32) -> f32 {
    (height * RING_DROPLETS_PER_METRE).min(MOST_RING_DROPLETS)
}

/// How fast a droplet leaves a point of the shore, where water comes in going `inwards`
/// (of length 1): up, and more so off a steep bank, which turns the water upwards; on in
/// the way it was going, and more so on a flat beach, which it runs up; and a little
/// either way along the shore.
fn shore_spray_velocity(random: &mut Random, inwards: Vec2, steepness: f32) -> Vec3 {
    let steep = (steepness / STEEP).min(1.0);
    let along = inwards.perp() * random.between(-0.6, 0.6);
    let on = inwards * random.between(0.4, 1.4) * (1.0 - 0.6 * steep);
    let across = on + along;
    Vec3::new(across.x, 0.0, across.y)
        + Vec3::Y * (random.between(1.0, 2.2) + steep * random.between(0.8, 2.5))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stronger_ring_throws_more_up_to_a_limit() {
        assert!(ring_droplets(0.02) > ring_droplets(0.01));
        assert_eq!(ring_droplets(10.0), MOST_RING_DROPLETS);
    }

    #[test]
    fn a_steep_bank_throws_more_and_higher() {
        assert_eq!(surf_droplets(0.0), SURF_DROPLETS[0]);
        assert_eq!(surf_droplets(5.0), SURF_DROPLETS[1]);
        let mut random = Random::default();
        let mean = |steepness: f32, random: &mut Random| {
            (0..500)
                .map(|_| shore_spray_velocity(random, Vec2::X, steepness))
                .sum::<Vec3>()
                / 500.0
        };
        let beach = mean(0.0, &mut random);
        let rock = mean(2.0, &mut random);
        assert!(rock.y > beach.y);
        // On a beach it runs on up the shore more.
        assert!(beach.x > rock.x && rock.x > 0.0);
    }
}
