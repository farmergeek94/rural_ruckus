//! A track's sky: a square picture (.RAW) with a palette of its own (.ACT), which the
//! level file names on lines 11 and 12, and the base game's weather skies. See
//! `docs/formats/level.md`.
//!
//! The picture is coloured in a way no other texture is. Its pixels use 16 palette slots
//! that the track's palette leaves black, and its colours are entries 192 to 207 of its
//! own palette, moved into those slots: a pixel of slot `p` is colour `192 + p - first`.
//! **Measured**: every pixel of the base game's four skies is in slots 230 to 245, and
//! every one of the 24 native track palettes leaves those black; the three tracks made
//! from MTM1 ones use slots 244 to 251 of 240 to 255, which their palette `METALCR2.ACT`
//! leaves black. **Reference** for the second (JSTrackViewer, `lvl-parser.js`, for the
//! engine MTM1 was built on): it copies the sky palette's entries 192 to 207 into slots
//! 240 to 255. Which of the two a sky uses is scored, by how many of its pixels fall in
//! the slots (`first_slot`).

use super::{Palette, Texture};

/// The first of the sky palette's 16 colours.
const SKY_COLOURS: usize = 192;
/// How many colours a sky has.
const COLOURS: usize = 16;
/// Where the sky's colours may go in the palette: the first of 16 slots.
const FIRST_SLOTS: [usize; 2] = [230, 240];

/// The base game's weather skies, in its `STARTUP.POD`. **Reference** (JSTrackViewer,
/// `scene.js` and its README): MTM2 offers clear, cloudy, dusk and night skies, and every
/// native level names `CLOUDY2`, the clear one. **Measured**: the other three are in
/// `STARTUP.POD`, 256 x 256, each with its palette beside it.
pub const CLOUDY_SKY: &str = "CCLOUDS.RAW";
pub const DUSK_SKY: &str = "DUSKSKY.RAW";
pub const NIGHT_SKY: &str = "NITESKY.RAW";

/// The first of the 16 slots a sky's pixels use: of `FIRST_SLOTS`, the one that holds the
/// most of them.
pub fn first_slot(texture: &Texture) -> usize {
    let size = texture.size();
    let held = |first: usize| {
        (0..size)
            .flat_map(|y| (0..size).map(move |x| (x, y)))
            .filter(|&(x, y)| (first..first + COLOURS).contains(&(texture.index(x, y) as usize)))
            .count()
    };
    FIRST_SLOTS
        .into_iter()
        .max_by_key(|&first| held(first))
        .unwrap_or(FIRST_SLOTS[0])
}

/// The sky as red, green, blue and (opaque) alpha bytes, row by row from the top, which is
/// the top of the sky. A pixel outside its 16 slots is black.
pub fn sky_rgba(texture: &Texture, palette: &Palette) -> Vec<u8> {
    let first = first_slot(texture);
    let size = texture.size();
    let mut rgba = Vec::with_capacity(size * size * 4);
    for y in 0..size {
        for x in 0..size {
            let slot = texture.index(x, y) as usize;
            let [red, green, blue] = if (first..first + COLOURS).contains(&slot) {
                palette.color((SKY_COLOURS + slot - first) as u8)
            } else {
                [0, 0, 0]
            };
            rgba.extend_from_slice(&[red, green, blue, 255]);
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A palette whose entry `i` is (i, 255 - i, 7).
    fn palette() -> Palette {
        let bytes: Vec<u8> = (0..=255u8).flat_map(|i| [i, 255 - i, 7]).collect();
        Palette::parse(&bytes).unwrap()
    }

    #[test]
    fn a_native_sky_takes_its_colours_from_192_up_into_slots_230_up() {
        let pixels: Vec<u8> = (0..16).map(|i| 230 + i as u8).collect();
        let texture = Texture::parse(&pixels).unwrap();
        assert_eq!(first_slot(&texture), 230);
        let rgba = sky_rgba(&texture, &palette());
        assert_eq!(&rgba[..4], &[192, 63, 7, 255]);
        assert_eq!(&rgba[15 * 4..16 * 4], &[207, 48, 7, 255]);
    }

    #[test]
    fn a_sky_in_the_top_slots_takes_them_instead() {
        // The eight slots an MTM1 sky uses, 244 to 251, fit either way; only 240 up holds
        // every one.
        let pixels: Vec<u8> = (0..16).map(|i| 244 + (i % 8) as u8).collect();
        let texture = Texture::parse(&pixels).unwrap();
        assert_eq!(first_slot(&texture), 240);
        assert_eq!(&sky_rgba(&texture, &palette())[..4], &[196, 59, 7, 255]);
    }

    #[test]
    fn a_pixel_outside_the_slots_is_black() {
        let mut pixels = vec![235u8; 16];
        pixels[3] = 12;
        let rgba = sky_rgba(&Texture::parse(&pixels).unwrap(), &palette());
        assert_eq!(&rgba[12..16], &[0, 0, 0, 255]);
    }
}
