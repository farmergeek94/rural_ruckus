//! The engine's three loops, from the base game's sounds.
//!
//! No file holds a loop alone. Each of the engine's files begins with the loop it leaves
//! and ends with the loop it goes to, and the loops are found as the samples that two
//! files share exactly: the idle loop is how `STARTIDL.WAV` and `M1-2-IDL.WAV` end, the
//! M1 loop how `IDLE2M1.WAV` and `M2-2-M1.WAV` end, and the M2 loop how `M1-2-M2.WAV` ends
//! and `M2-2-M1.WAV` begins. **Measured**: see
//! `docs/formats/sound.md`, which has the loops' lengths in `SOUND.POD`.

use std::sync::Arc;

use crate::Wave;
use crate::base_game::BaseGame;
use content::sound::{EngineLoops, Loop};

/// The shortest a loop may be, in seconds, to be taken for one. The shortest found is
/// 1.08 s; two files that share less than this do not share a loop, and something is
/// wrong with them.
const SHORTEST_LOOP: f32 = 0.5;

/// The engine's loops, from whichever base archives hold its files.
pub fn engine_loops(base: &BaseGame) -> Result<EngineLoops, String> {
    let start = read(base, "STARTIDL.WAV")?;
    let idle_to_middle = read(base, "IDLE2M1.WAV")?;
    let middle_to_idle = read(base, "M1-2-IDL.WAV")?;
    let middle_to_high = read(base, "M1-2-M2.WAV")?;
    let high_to_middle = read(base, "M2-2-M1.WAV")?;

    let rate = start.sample_rate;
    let files = [
        &idle_to_middle,
        &middle_to_idle,
        &middle_to_high,
        &high_to_middle,
    ];
    if files.iter().any(|wave| wave.sample_rate != rate) {
        return Err("the engine's sounds are not all at one sample rate".into());
    }
    let shortest = (SHORTEST_LOOP * rate as f32) as usize;
    let as_loop = |samples: &[f32], name: &str| {
        if samples.len() < shortest {
            return Err(format!("the engine's {name} loop is not found"));
        }
        Ok(Loop {
            sample_rate: rate,
            samples: Arc::from(samples),
        })
    };

    let [
        start,
        idle_to_middle,
        middle_to_idle,
        middle_to_high,
        high_to_middle,
    ] = [
        &start,
        &idle_to_middle,
        &middle_to_idle,
        &middle_to_high,
        &high_to_middle,
    ]
    .map(mono);
    let idle = shared_end(&start, &middle_to_idle);
    let m1 = shared_end(&idle_to_middle, &high_to_middle);
    let m2 = end_that_begins(&middle_to_high, &high_to_middle);
    Ok(EngineLoops {
        idle: as_loop(idle, "idle")?,
        m1: as_loop(m1, "M1")?,
        m2: as_loop(m2, "M2")?,
    })
}

/// A base game file, read on its own: the rest of its archive is not.
fn read(base: &BaseGame, name: &str) -> Result<Wave, String> {
    let (archive, entry) = base
        .directories()
        .find_map(|(archive, entries)| {
            let entry = entries
                .iter()
                .find(|entry| entry.file_name().eq_ignore_ascii_case(name))?;
            Some((archive, entry))
        })
        .ok_or_else(|| format!("{name} is in none of the base game's archives"))?;
    let bytes = base.read_one(archive, entry)?;
    Wave::parse(&bytes).map_err(|error| format!("{name}: {error}"))
}

/// A sound's samples, from -1 to 1, its channels mixed into one.
fn mono(wave: &Wave) -> Vec<f32> {
    let channels = usize::from(wave.channels.max(1));
    wave.samples
        .chunks(channels)
        .map(|frame| {
            frame.iter().map(|&sample| f32::from(sample)).sum::<f32>()
                / 32768.0
                / frame.len() as f32
        })
        .collect()
}

/// The longest run of samples that both `a` and `b` end with, as `a` has it.
fn shared_end<'a>(a: &'a [f32], b: &[f32]) -> &'a [f32] {
    let shared = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    &a[a.len() - shared..]
}

/// The longest run of samples that `a` ends with and `b` begins with, as `a` has it.
fn end_that_begins<'a>(a: &'a [f32], b: &[f32]) -> &'a [f32] {
    // From the earliest start, so that the first match is the longest. A wrong start
    // nearly always differs within a few samples.
    (a.len().saturating_sub(b.len())..a.len())
        .find(|&from| a[from..] == b[..a.len() - from])
        .map_or(&a[a.len()..], |from| &a[from..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_loop_is_what_two_files_end_with() {
        let a = [0.9, 0.1, 0.2, 0.3];
        let b = [0.5, 0.6, 0.7, 0.1, 0.2, 0.3];
        assert_eq!(shared_end(&a, &b), &[0.1, 0.2, 0.3]);
        assert!(shared_end(&a, &[0.4]).is_empty());
    }

    #[test]
    fn a_loop_is_what_one_file_ends_with_and_another_begins_with() {
        let a = [0.9, 0.8, 0.1, 0.2, 0.3];
        let b = [0.1, 0.2, 0.3, 0.7, 0.6];
        assert_eq!(end_that_begins(&a, &b), &[0.1, 0.2, 0.3]);
        // The longest, where a shorter overlap would also do.
        let a = [0.5, 0.1, 0.1, 0.1];
        let b = [0.1, 0.1, 0.1, 0.4];
        assert_eq!(end_that_begins(&a, &b), &[0.1, 0.1, 0.1]);
        assert!(end_that_begins(&[0.5], &[0.4]).is_empty());
    }

    #[test]
    fn channels_are_mixed_into_one() {
        let wave = Wave {
            sample_rate: 11025,
            channels: 2,
            samples: vec![16384, 0, -32768, -32768],
        };
        assert_eq!(mono(&wave), vec![0.25, -1.0]);
    }
}
