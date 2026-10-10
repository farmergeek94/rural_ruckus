//! True-colour textures: the PNG or TGA that MTM2 Community Patch 3 packs beside a
//! texture's .RAW, or in place of it, and the normal maps beside them. See
//! `docs/formats/textures.md`.
//!
//! Files still name the texture by its .RAW name; the image is found by that name's stem.
//! It is found and checked here, but not decoded, since this module uses nothing but
//! `std` and decoding a PNG takes a library. `crate::hd_texture` decodes both kinds.

use super::{PodArchive, PodError, folder};

/// The eight bytes every PNG file starts with (PNG specification, section 5.2).
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];

/// The length of a TGA file's header, which has no signature (Truevision TGA 2.0).
const TGA_HEADER: usize = 18;
/// TGA image types: true colour, uncompressed and run-length encoded. The patched game
/// takes no other. **Reference:** JSTruckViewer, `src/worker/image-decoder.js`.
const TGA_TRUE_COLOR: [u8; 2] = [2, 10];

/// The sizes the patched game takes, in pixels along a side. **Reference:** JSTrackViewer,
/// `src/worker/image-decoder.js`, which has them from the Traxx editor's fork.
const SMALLEST: usize = 32;
const LARGEST: usize = 1024;

/// Where a texture's image is looked for first, in order. Any other folder comes after.
/// **Reference:** JSTrackViewer, `src/worker/palette-resolver.js`.
const FOLDERS: [&str; 4] = [folder::ART, folder::MODELS, folder::DATA, "TEXTURES"];

/// What a normal map's name adds to its texture's stem. **Reference:** JSTruckViewer,
/// `docs/BIN_HD_FORMAT.md`.
const NORMAL_MAP_SUFFIX: &str = "_N";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdFormat {
    Png,
    Tga,
}

/// An image file, still encoded, known to be square and of a size the patched game takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HdTexture {
    format: HdFormat,
    size: usize,
    bytes: Vec<u8>,
}

impl HdTexture {
    /// Checks the signature and reads the size from the header chunk (IHDR), which the PNG
    /// specification puts first: 4 bytes of length, `IHDR`, then width and height as
    /// big-endian 32-bit numbers.
    pub fn parse_png(bytes: &[u8]) -> Result<Self, PodError> {
        if !bytes.starts_with(&PNG_SIGNATURE) || bytes.get(12..16) != Some(b"IHDR") {
            return Err(PodError::Unsupported(
                "a file named .PNG that isn't a PNG".into(),
            ));
        }
        let number = |offset: usize| {
            bytes
                .get(offset..offset + 4)
                .map(|field| u32::from_be_bytes(field.try_into().expect("four bytes")) as usize)
        };
        let (Some(width), Some(height)) = (number(16), number(20)) else {
            return Err(PodError::Truncated {
                what: "PNG header".into(),
                offset: 16,
                needed: 8,
                available: bytes.len().saturating_sub(16),
            });
        };
        Self::checked(HdFormat::Png, width, height, bytes)
    }

    /// Reads the size from the header: image type at byte 2, width and height as
    /// little-endian 16-bit numbers at 12 and 14, bits per pixel at 16. A TGA has no
    /// signature, so a file that starts like another kind of image is refused rather than
    /// read as one.
    pub fn parse_tga(bytes: &[u8]) -> Result<Self, PodError> {
        if bytes.starts_with(&PNG_SIGNATURE) || bytes.starts_with(&[0xff, 0xd8, 0xff]) {
            return Err(PodError::Unsupported(
                "a file named .TGA that is another kind of image".into(),
            ));
        }
        let Some(header) = bytes.get(..TGA_HEADER) else {
            return Err(PodError::Truncated {
                what: "TGA header".into(),
                offset: 0,
                needed: TGA_HEADER,
                available: bytes.len(),
            });
        };
        let has_color_map = header[1] != 0;
        if has_color_map || !TGA_TRUE_COLOR.contains(&header[2]) || ![24, 32].contains(&header[16])
        {
            return Err(PodError::Unsupported(
                "a TGA that isn't 24 or 32-bit true colour".into(),
            ));
        }
        let number = |offset: usize| u16::from_le_bytes([header[offset], header[offset + 1]]);
        Self::checked(
            HdFormat::Tga,
            number(12) as usize,
            number(14) as usize,
            bytes,
        )
    }

    fn checked(
        format: HdFormat,
        width: usize,
        height: usize,
        bytes: &[u8],
    ) -> Result<Self, PodError> {
        if width != height || !width.is_power_of_two() || !(SMALLEST..=LARGEST).contains(&width) {
            return Err(PodError::Unsupported(format!(
                "a {width} x {height} image: the game takes square powers of two, \
                 {SMALLEST} to {LARGEST} pixels"
            )));
        }
        Ok(Self {
            format,
            size: width,
            bytes: bytes.to_vec(),
        })
    }

    pub fn format(&self) -> HdFormat {
        self.format
    }

    /// Pixels along each side.
    pub fn size(&self) -> usize {
        self.size
    }

    /// The file as it is in the archive.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// The image that stands in for the texture a file names, such as `SKTK002.RAW`, if the
/// archive has one that the game can use.
pub(super) fn find(archive: &PodArchive, texture_name: &str) -> Option<HdTexture> {
    find_stem(archive, stem(texture_name), "")
}

/// The normal map for the texture a file names: `SKTK002_N.PNG` for `SKTK002.RAW`.
pub(super) fn find_normal_map(archive: &PodArchive, texture_name: &str) -> Option<HdTexture> {
    find_stem(archive, stem(texture_name), NORMAL_MAP_SUFFIX)
}

fn stem(texture_name: &str) -> &str {
    let file_name = texture_name
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(texture_name);
    file_name
        .rsplit_once('.')
        .map_or(file_name, |(stem, _)| stem)
}

/// A PNG anywhere comes before a TGA, and within each, the copy in the earliest folder of
/// `FOLDERS`. One pass over the archive.
fn find_stem(archive: &PodArchive, stem: &str, suffix: &str) -> Option<HdTexture> {
    let names = [HdFormat::Png, HdFormat::Tga].map(|format| {
        let extension = match format {
            HdFormat::Png => "PNG",
            HdFormat::Tga => "TGA",
        };
        (format, format!("{stem}{suffix}.{extension}"))
    });
    let folder_rank = |folder: &str| {
        FOLDERS
            .iter()
            .position(|known| known.eq_ignore_ascii_case(folder))
            .unwrap_or(FOLDERS.len())
    };
    let (_, format, entry) = archive
        .entries()
        .iter()
        .filter_map(|entry| {
            let rank = names
                .iter()
                .position(|(_, name)| entry.file_name().eq_ignore_ascii_case(name))?;
            Some((rank, names[rank].0, entry))
        })
        .min_by_key(|(rank, _, entry)| (*rank, folder_rank(entry.folder())))?;
    match format {
        HdFormat::Png => HdTexture::parse_png(archive.data(entry)).ok(),
        HdFormat::Tga => HdTexture::parse_tga(archive.data(entry)).ok(),
    }
}

/// Whether the archive holds any PNG or TGA at all. Most don't, and needn't be searched
/// for one texture at a time.
pub(super) fn any_in(archive: &PodArchive) -> bool {
    archive.entries().iter().any(|entry| {
        entry.extension().eq_ignore_ascii_case("png")
            || entry.extension().eq_ignore_ascii_case("tga")
    })
}

#[cfg(test)]
mod tests {
    use super::super::archive::tests::build_archive;
    use super::*;

    /// The start of a PNG: signature and header chunk, which is all `parse_png` reads.
    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = PNG_SIGNATURE.to_vec();
        bytes.extend(13u32.to_be_bytes());
        bytes.extend(b"IHDR");
        bytes.extend(width.to_be_bytes());
        bytes.extend(height.to_be_bytes());
        bytes.extend([8, 6, 0, 0, 0]);
        bytes
    }

    /// A TGA header, which is all `parse_tga` reads.
    fn tga(image_type: u8, size: u16, bits: u8) -> Vec<u8> {
        let mut bytes = vec![0, 0, image_type, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        bytes.extend(size.to_le_bytes());
        bytes.extend(size.to_le_bytes());
        bytes.extend([bits, 0x20]);
        bytes
    }

    #[test]
    fn reads_the_size() {
        let texture = HdTexture::parse_png(&png(64, 64)).unwrap();
        assert_eq!((texture.format(), texture.size()), (HdFormat::Png, 64));
        assert_eq!(texture.bytes(), png(64, 64));

        let texture = HdTexture::parse_tga(&tga(10, 128, 32)).unwrap();
        assert_eq!((texture.format(), texture.size()), (HdFormat::Tga, 128));
    }

    #[test]
    fn refuses_what_the_game_would_not_take() {
        for (width, height) in [(64, 32), (48, 48), (16, 16), (2048, 2048)] {
            assert!(
                HdTexture::parse_png(&png(width, height)).is_err(),
                "{width} x {height}"
            );
        }
        let mut jpeg = png(64, 64);
        jpeg[..3].copy_from_slice(&[0xff, 0xd8, 0xff]);
        assert!(HdTexture::parse_png(&jpeg).is_err());
        assert!(HdTexture::parse_png(&PNG_SIGNATURE).is_err());

        // A paletted TGA, a 16-bit one, a PNG named .TGA, and a header cut short.
        assert!(HdTexture::parse_tga(&tga(1, 64, 8)).is_err());
        assert!(HdTexture::parse_tga(&tga(2, 64, 16)).is_err());
        assert!(HdTexture::parse_tga(&png(64, 64)).is_err());
        assert!(HdTexture::parse_tga(&tga(2, 64, 24)[..10]).is_err());
    }

    #[test]
    fn finds_the_image_by_the_raw_names_stem() {
        let data = png(64, 64);
        let archive = PodArchive::parse(build_archive("", &[("ART\\GRASS.PNG", &data)])).unwrap();
        assert!(find(&archive, "grass.raw").is_some());
        assert!(find(&archive, "ART\\GRASS.RAW").is_some());
        assert!(find(&archive, "ROAD.RAW").is_none());
    }

    #[test]
    fn prefers_a_png_then_the_art_folder() {
        let (small, large, targa) = (png(32, 32), png(64, 64), tga(2, 128, 24));
        let archive = PodArchive::parse(build_archive(
            "",
            &[
                ("ART\\GRASS.TGA", &targa),
                ("OTHER\\GRASS.PNG", &small),
                ("ART\\GRASS.PNG", &large),
                ("ART\\ROAD.TGA", &targa),
            ],
        ))
        .unwrap();
        assert_eq!(find(&archive, "GRASS.RAW").unwrap().size(), 64);
        assert_eq!(find(&archive, "ROAD.RAW").unwrap().format(), HdFormat::Tga);
    }

    #[test]
    fn finds_a_textures_normal_map() {
        let (color, normal) = (png(64, 64), png(128, 128));
        let archive = PodArchive::parse(build_archive(
            "",
            &[("ART\\BODY.PNG", &color), ("ART\\BODY_N.PNG", &normal)],
        ))
        .unwrap();
        assert_eq!(find(&archive, "body.raw").unwrap().size(), 64);
        assert_eq!(find_normal_map(&archive, "body.raw").unwrap().size(), 128);
        assert!(find_normal_map(&archive, "road.raw").is_none());
    }
}
