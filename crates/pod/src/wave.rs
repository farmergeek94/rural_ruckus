//! Sounds (.WAV): RIFF files of uncompressed PCM. Every MTM2 sound measured is one channel
//! of 8-bit samples, at 11 025 or 22 050 samples a second; 16-bit samples and more channels
//! are read too, as the RIFF format has them. See `docs/formats/sound.md`.

use super::PodError;

/// The RIFF format code for uncompressed PCM.
const PCM: u16 = 1;

/// A sound, as its file holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wave {
    /// Samples a second, of each channel.
    pub sample_rate: u32,
    /// How many channels the samples are interleaved from.
    pub channels: u16,
    /// Every sample, interleaved by channel, on a 16-bit scale: an 8-bit file's unsigned
    /// samples, whose silence is 128, are moved to silence at 0 and multiplied by 256.
    pub samples: Vec<i16>,
}

impl Wave {
    pub fn parse(bytes: &[u8]) -> Result<Self, PodError> {
        let truncated = |what: &str, offset: usize, needed: usize| PodError::Truncated {
            what: what.into(),
            offset,
            needed,
            available: bytes.len().saturating_sub(offset),
        };
        if bytes.len() < 12 {
            return Err(truncated("RIFF header", 0, 12));
        }
        if &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
            return Err(PodError::Unsupported(
                "a sound that is not RIFF WAVE".into(),
            ));
        }

        // (format, channels, sample rate, bits a sample), then the samples' bytes.
        let mut format = None;
        let mut data = None;
        let mut at = 12;
        while at + 8 <= bytes.len() {
            let id = &bytes[at..at + 4];
            let size = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
            let body = at + 8;
            if id == b"fmt " {
                if body + 16 > bytes.len() {
                    return Err(truncated("format chunk", body, 16));
                }
                let u16_at = |offset: usize| {
                    u16::from_le_bytes(bytes[body + offset..body + offset + 2].try_into().unwrap())
                };
                let rate = u32::from_le_bytes(bytes[body + 4..body + 8].try_into().unwrap());
                format = Some((u16_at(0), u16_at(2), rate, u16_at(14)));
            } else if id == b"data" {
                // A data chunk that says it is longer than the file is read to the end of
                // the file.
                data = Some(&bytes[body..(body + size).min(bytes.len())]);
            }
            // Chunks are padded to an even length.
            at = body.saturating_add(size).saturating_add(size & 1);
        }

        let Some((code, channels, sample_rate, bits)) = format else {
            return Err(PodError::MissingField {
                field: "format chunk".into(),
            });
        };
        let Some(data) = data else {
            return Err(PodError::MissingField {
                field: "data chunk".into(),
            });
        };
        if code != PCM {
            return Err(PodError::Unsupported(format!("sound format {code}")));
        }
        if channels == 0 || sample_rate == 0 {
            return Err(PodError::BadValue {
                field: "channels and sample rate".into(),
                line: 0,
                text: format!("{channels} channels at {sample_rate} a second"),
            });
        }
        let samples = match bits {
            8 => data
                .iter()
                .map(|&sample| (i16::from(sample) - 128) * 256)
                .collect(),
            16 => data
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&pair| i16::from_le_bytes(pair))
                .collect(),
            _ => return Err(PodError::Unsupported(format!("{bits}-bit sound"))),
        };
        Ok(Self {
            sample_rate,
            channels,
            samples,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A RIFF WAVE file with these chunks after its header.
    fn riff(chunks: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
        let mut body = b"WAVE".to_vec();
        for (id, data) in chunks {
            body.extend_from_slice(*id);
            body.extend_from_slice(&(data.len() as u32).to_le_bytes());
            body.extend_from_slice(data);
            if data.len() % 2 == 1 {
                body.push(0);
            }
        }
        let mut file = b"RIFF".to_vec();
        file.extend_from_slice(&(body.len() as u32).to_le_bytes());
        file.extend(body);
        file
    }

    fn format(code: u16, channels: u16, rate: u32, bits: u16) -> Vec<u8> {
        let align = channels * bits / 8;
        let mut chunk = Vec::new();
        chunk.extend_from_slice(&code.to_le_bytes());
        chunk.extend_from_slice(&channels.to_le_bytes());
        chunk.extend_from_slice(&rate.to_le_bytes());
        chunk.extend_from_slice(&(rate * u32::from(align)).to_le_bytes());
        chunk.extend_from_slice(&align.to_le_bytes());
        chunk.extend_from_slice(&bits.to_le_bytes());
        chunk
    }

    #[test]
    fn eight_bit_samples_are_centred_on_silence() {
        let file = riff(&[
            (b"fmt ", format(1, 1, 11025, 8)),
            (b"data", vec![128, 255, 0, 129, 127]),
        ]);
        let wave = Wave::parse(&file).unwrap();
        assert_eq!((wave.sample_rate, wave.channels), (11025, 1));
        assert_eq!(wave.samples, vec![0, 127 * 256, -128 * 256, 256, -256]);
    }

    #[test]
    fn sixteen_bit_samples_are_read_as_they_are() {
        let file = riff(&[
            (b"fmt ", format(1, 2, 22050, 16)),
            (
                b"data",
                [1i16, -2, 300, -32768]
                    .iter()
                    .flat_map(|s| s.to_le_bytes())
                    .collect(),
            ),
        ]);
        let wave = Wave::parse(&file).unwrap();
        assert_eq!((wave.sample_rate, wave.channels), (22050, 2));
        assert_eq!(wave.samples, vec![1, -2, 300, -32768]);
    }

    #[test]
    fn chunks_after_the_data_and_odd_lengths_are_stepped_over() {
        // As some of SOUND.POD's files have: `LIST` and `fact` after the data.
        let file = riff(&[
            (b"fmt ", format(1, 1, 11025, 8)),
            (b"data", vec![130, 126, 128]),
            (b"LIST", vec![1, 2, 3]),
            (b"fact", vec![3, 0, 0, 0]),
        ]);
        assert_eq!(Wave::parse(&file).unwrap().samples, vec![512, -512, 0]);
    }

    #[test]
    fn a_data_chunk_longer_than_the_file_is_read_to_its_end() {
        let mut file = riff(&[(b"fmt ", format(1, 1, 11025, 8)), (b"data", vec![129, 129])]);
        let size_at = file.len() - 6;
        file[size_at..size_at + 4].copy_from_slice(&100u32.to_le_bytes());
        assert_eq!(Wave::parse(&file).unwrap().samples, vec![256, 256]);
    }

    #[test]
    fn what_is_not_pcm_wave_is_refused_without_a_panic() {
        assert!(Wave::parse(b"RIFF").is_err());
        assert!(Wave::parse(b"RIFF\0\0\0\0AVI LIST").is_err());
        let compressed = riff(&[(b"fmt ", format(2, 1, 11025, 4)), (b"data", vec![0; 4])]);
        assert!(Wave::parse(&compressed).is_err());
        let no_data = riff(&[(b"fmt ", format(1, 1, 11025, 8))]);
        assert!(Wave::parse(&no_data).is_err());
        let short_format = riff(&[(b"fmt ", vec![1, 0, 1])]);
        assert!(Wave::parse(&short_format).is_err());
        let no_rate = riff(&[(b"fmt ", format(1, 1, 0, 8)), (b"data", vec![128])]);
        assert!(Wave::parse(&no_rate).is_err());
    }
}
