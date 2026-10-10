//! Smaller copies of a texture, for drawing it at a distance without shimmering.
//!
//! Made here on the CPU, for textures known when a track or a truck loads. Bevy's own
//! generator runs on the graphics card and handles neither sRGB formats nor array textures.
//! Used by the ground and scenery (`track::tile_array`) and by the truck's paintwork and
//! normal maps.

/// A square RGBA image followed by each of its mipmaps, halving down to a single pixel.
/// Only sizes that halve evenly all the way get mipmaps; anything else is left as it is.
pub fn mip_chain(rgba: &[u8], size: usize) -> Vec<Vec<u8>> {
    let mut chain = vec![rgba.to_vec()];
    if !size.is_power_of_two() {
        return chain;
    }
    let mut size = size;
    while size > 1 {
        let smaller = halve(chain.last().expect("starts with one level"), size);
        chain.push(smaller);
        size /= 2;
    }
    chain
}

/// The same for a texture that holds numbers rather than colours, such as a normal map's
/// directions: each channel of each level is the plain average of the level above. The
/// shader makes an averaged normal a unit one again.
pub fn data_mip_chain(rgba: &[u8], size: usize) -> Vec<Vec<u8>> {
    let mut chain = vec![rgba.to_vec()];
    if !size.is_power_of_two() {
        return chain;
    }
    let mut size = size;
    while size > 1 {
        let larger = chain.last().expect("starts with one level");
        let half = size / 2;
        let mut smaller = Vec::with_capacity(half * half * 4);
        for y in 0..half {
            for x in 0..half {
                for channel in 0..4 {
                    let sum: u32 = [(0, 0), (1, 0), (0, 1), (1, 1)]
                        .iter()
                        .map(|(dx, dy)| {
                            larger[((2 * y + dy) * size + 2 * x + dx) * 4 + channel] as u32
                        })
                        .sum();
                    smaller.push(((sum + 2) / 4) as u8);
                }
            }
        }
        chain.push(smaller);
        size = half;
    }
    chain
}

/// Each pixel of the result is the average of a 2 x 2 block. The colours are averaged as
/// light, not as the sRGB numbers that store them: averaging those directly comes out
/// too dark, and would make the ground dim with distance. They are also weighted by
/// alpha, so that the holes in a cutout texture don't darken the edges round them.
fn halve(rgba: &[u8], size: usize) -> Vec<u8> {
    let half = size / 2;
    let mut smaller = Vec::with_capacity(half * half * 4);
    for y in 0..half {
        for x in 0..half {
            let block = [(0, 0), (1, 0), (0, 1), (1, 1)]
                .map(|(dx, dy)| ((2 * y + dy) * size + 2 * x + dx) * 4);
            let alpha: u32 = block.iter().map(|&pixel| rgba[pixel + 3] as u32).sum();
            for channel in 0..3 {
                let light: f32 = block
                    .iter()
                    .map(|&pixel| srgb_to_linear(rgba[pixel + channel]) * rgba[pixel + 3] as f32)
                    .sum();
                smaller.push(linear_to_srgb(if alpha > 0 {
                    light / alpha as f32
                } else {
                    0.0
                }));
            }
            // Alpha is a plain proportion, not a colour.
            smaller.push(((alpha + 2) / 4) as u8);
        }
    }
    smaller
}

fn srgb_to_linear(value: u8) -> f32 {
    let value = value as f32 / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(light: f32) -> u8 {
    let value = if light <= 0.003_130_8 {
        light * 12.92
    } else {
        1.055 * light.powf(1.0 / 2.4) - 0.055
    };
    (value * 255.0).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_data_texture_averages_its_numbers_as_they_are() {
        // A normal map's "straight out" and "tilted", side by side over two rows.
        let tile = [[128, 128, 255, 255], [255, 128, 128, 255]]
            .concat()
            .repeat(2);
        let chain = data_mip_chain(&tile, 2);
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[1], [192, 128, 192, 255]);
    }

    #[test]
    fn mip_chain_halves_down_to_one_pixel() {
        let chain = mip_chain(&[200; 8 * 8 * 4], 8);
        let sizes: Vec<usize> = chain.iter().map(Vec::len).collect();
        assert_eq!(sizes, [8 * 8 * 4, 4 * 4 * 4, 2 * 2 * 4, 4]);
        // A flat colour stays that colour at every size.
        assert!(chain.iter().flatten().all(|&byte| byte == 200));
    }

    #[test]
    fn sizes_that_do_not_halve_evenly_get_no_mipmaps() {
        assert_eq!(mip_chain(&[0; 6 * 6 * 4], 6).len(), 1);
    }

    #[test]
    fn averages_light_rather_than_srgb_numbers() {
        // Black and white in equal parts. Half the light is sRGB 188, not 128.
        let checks = [0, 0, 0, 255, 255, 255, 255, 255].repeat(2);
        assert_eq!(halve(&checks, 2), [188, 188, 188, 255]);
    }

    #[test]
    fn holes_do_not_darken_their_surroundings() {
        // One white pixel beside three holes: a quarter there, and still white.
        let mut block = [0, 0, 0, 0].repeat(4);
        block[..4].copy_from_slice(&[255, 255, 255, 255]);
        assert_eq!(halve(&block, 2), [255, 255, 255, 64]);
        assert_eq!(halve(&[0, 0, 0, 0].repeat(4), 2), [0, 0, 0, 0]);
    }

    #[test]
    fn srgb_conversion_round_trips() {
        for value in 0..=255 {
            assert_eq!(linear_to_srgb(srgb_to_linear(value)), value);
        }
    }
}
