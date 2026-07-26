//! Strict character-encoding support at Grayslate's file boundary.
//!
//! The editor always holds Unicode text. Encoding and line endings are
//! document metadata and are applied only when bytes cross the disk boundary.

use std::borrow::Cow;

use encoding_rs::{DecoderResult, EncoderResult, WINDOWS_1252};
use serde::{Deserialize, Serialize};

use crate::line_ending::Eol;

const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
const UTF16_LE_BOM: &[u8] = &[0xFF, 0xFE];
const UTF16_BE_BOM: &[u8] = &[0xFE, 0xFF];
const MAX_PROBE_BYTES: usize = 64 * 1024;
pub const MAX_DECODED_TEXT_SIZE: usize = 200 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum CharacterEncoding {
    #[default]
    #[serde(rename = "utf-8")]
    Utf8,
    #[serde(rename = "utf-8-bom")]
    Utf8Bom,
    #[serde(rename = "utf-16le")]
    Utf16Le,
    #[serde(rename = "utf-16be")]
    Utf16Be,
    #[serde(rename = "windows-1252")]
    Windows1252,
}

impl CharacterEncoding {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Utf8 => "utf-8",
            Self::Utf8Bom => "utf-8-bom",
            Self::Utf16Le => "utf-16le",
            Self::Utf16Be => "utf-16be",
            Self::Windows1252 => "windows-1252",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "utf-8" => Ok(Self::Utf8),
            "utf-8-bom" => Ok(Self::Utf8Bom),
            "utf-16le" => Ok(Self::Utf16Le),
            "utf-16be" => Ok(Self::Utf16Be),
            "windows-1252" => Ok(Self::Windows1252),
            other => Err(format!("Unsupported character encoding: {other}")),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextFormat {
    pub eol: Eol,
    pub encoding: CharacterEncoding,
}

impl From<Eol> for TextFormat {
    fn from(eol: Eol) -> Self {
        Self {
            eol,
            encoding: CharacterEncoding::Utf8,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EncodingChoiceReason {
    LegacySingleByte,
    BomlessUtf16,
}

#[derive(Debug, Eq, PartialEq)]
pub enum DecodeDecision {
    Decoded {
        text: String,
        encoding: CharacterEncoding,
    },
    NeedsConfirmation {
        suggested_encoding: CharacterEncoding,
        reason: EncodingChoiceReason,
    },
}

fn has_utf32_bom(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x00, 0x00, 0xFE, 0xFF]) || bytes.starts_with(&[0xFF, 0xFE, 0x00, 0x00])
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> Result<String, String> {
    if bytes.len() % 2 != 0 {
        return Err("UTF-16 input has an incomplete trailing code unit.".to_string());
    }

    let units = bytes.chunks_exact(2).map(|pair| {
        if little_endian {
            u16::from_le_bytes([pair[0], pair[1]])
        } else {
            u16::from_be_bytes([pair[0], pair[1]])
        }
    });
    let mut text = String::with_capacity(bytes.len().min(MAX_DECODED_TEXT_SIZE));
    for decoded in char::decode_utf16(units) {
        let character =
            decoded.map_err(|_| "UTF-16 input contains an unpaired surrogate.".to_string())?;
        if text.len() + character.len_utf8() > MAX_DECODED_TEXT_SIZE {
            return Err("Decoded text exceeds the maximum supported size of 200 MB.".to_string());
        }
        text.push(character);
    }
    Ok(text)
}

fn decode_windows_1252(bytes: &[u8]) -> Result<String, String> {
    if !is_valid_windows_1252(bytes) {
        return Err("The file contains bytes that are undefined in Windows-1252.".to_string());
    }
    let capacity = bytes.iter().try_fold(0_usize, |length, byte| {
        let encoded_length = match byte {
            0x00..=0x7F => 1,
            0x83 | 0x88 | 0x8A | 0x8C | 0x8E | 0x98 | 0x9A | 0x9C | 0x9E | 0x9F | 0xA0..=0xFF => 2,
            _ => 3,
        };
        length
            .checked_add(encoded_length)
            .filter(|total| *total <= MAX_DECODED_TEXT_SIZE)
            .ok_or_else(|| "Decoded text exceeds the maximum supported size of 200 MB.".to_string())
    })?;
    let mut decoder = WINDOWS_1252.new_decoder_without_bom_handling();
    // encoding_rs may require one maximum-width scalar of spare capacity even
    // when the exact decoded length is known.
    let mut text = String::with_capacity(capacity.saturating_add(3));
    let (result, _) = decoder.decode_to_string_without_replacement(bytes, &mut text, true);
    match result {
        DecoderResult::InputEmpty => Ok(text),
        DecoderResult::OutputFull => {
            Err("Decoded text exceeds the maximum supported size of 200 MB.".to_string())
        }
        DecoderResult::Malformed(_, _) => {
            Err("The file contains bytes that are undefined in Windows-1252.".to_string())
        }
    }
}

fn decode_as(bytes: &[u8], encoding: CharacterEncoding) -> Result<String, String> {
    if has_utf32_bom(bytes) {
        return Err("UTF-32 files are not supported.".to_string());
    }

    match encoding {
        CharacterEncoding::Utf8 => {
            if bytes.starts_with(UTF8_BOM) {
                return Err("This file has a UTF-8 BOM. Reopen it as UTF-8 with BOM.".to_string());
            }
            std::str::from_utf8(bytes)
                .map(str::to_owned)
                .map_err(|_| "The file is not valid UTF-8.".to_string())
        }
        CharacterEncoding::Utf8Bom => {
            let content = bytes.strip_prefix(UTF8_BOM).unwrap_or(bytes);
            std::str::from_utf8(content)
                .map(str::to_owned)
                .map_err(|_| "The file is not valid UTF-8.".to_string())
        }
        CharacterEncoding::Utf16Le => {
            if bytes.starts_with(UTF16_BE_BOM) {
                return Err("The file has a UTF-16 BE BOM, not UTF-16 LE.".to_string());
            }
            decode_utf16(bytes.strip_prefix(UTF16_LE_BOM).unwrap_or(bytes), true)
        }
        CharacterEncoding::Utf16Be => {
            if bytes.starts_with(UTF16_LE_BOM) {
                return Err("The file has a UTF-16 LE BOM, not UTF-16 BE.".to_string());
            }
            decode_utf16(bytes.strip_prefix(UTF16_BE_BOM).unwrap_or(bytes), false)
        }
        CharacterEncoding::Windows1252 => decode_windows_1252(bytes),
    }
}

fn looks_like_utf16(bytes: &[u8], little_endian: bool) -> bool {
    let sample = &bytes[..bytes.len().min(MAX_PROBE_BYTES)];
    if sample.len() < 4 || sample.len() % 2 != 0 {
        return false;
    }

    let pairs = sample.chunks_exact(2);
    let pair_count = pairs.len();
    let mut expected_nuls = 0_usize;
    let mut unexpected_nuls = 0_usize;
    for pair in pairs {
        let (text_byte, nul_byte) = if little_endian {
            (pair[0], pair[1])
        } else {
            (pair[1], pair[0])
        };
        expected_nuls += usize::from(nul_byte == 0 && text_byte != 0);
        unexpected_nuls += usize::from(text_byte == 0);
    }

    expected_nuls * 10 >= pair_count * 3
        && unexpected_nuls * 20 <= pair_count
        && decode_utf16(sample, little_endian).is_ok()
}

fn is_valid_windows_1252(bytes: &[u8]) -> bool {
    !bytes
        .iter()
        .any(|byte| matches!(byte, 0x81 | 0x8D | 0x8F | 0x90 | 0x9D))
}

fn looks_binary(bytes: &[u8]) -> bool {
    let sample = &bytes[..bytes.len().min(MAX_PROBE_BYTES)];
    if sample.contains(&0) {
        return true;
    }
    if sample.is_empty() {
        return false;
    }

    let suspicious = sample
        .iter()
        .filter(|&&byte| byte < 0x20 && !matches!(byte, b'\t' | b'\n' | b'\r' | 0x0C))
        .count();
    suspicious * 100 > sample.len() * 5
}

/// Detect and strictly decode bytes. Ambiguous legacy encodings are never
/// silently selected: the caller must ask the user before retrying explicitly.
pub fn detect_and_decode(bytes: &[u8]) -> Result<DecodeDecision, String> {
    if has_utf32_bom(bytes) {
        return Err("UTF-32 files are not supported.".to_string());
    }
    if bytes.starts_with(UTF8_BOM) {
        return Ok(DecodeDecision::Decoded {
            text: decode_as(bytes, CharacterEncoding::Utf8Bom)?,
            encoding: CharacterEncoding::Utf8Bom,
        });
    }
    if bytes.starts_with(UTF16_LE_BOM) {
        return Ok(DecodeDecision::Decoded {
            text: decode_as(bytes, CharacterEncoding::Utf16Le)?,
            encoding: CharacterEncoding::Utf16Le,
        });
    }
    if bytes.starts_with(UTF16_BE_BOM) {
        return Ok(DecodeDecision::Decoded {
            text: decode_as(bytes, CharacterEncoding::Utf16Be)?,
            encoding: CharacterEncoding::Utf16Be,
        });
    }
    if looks_like_utf16(bytes, true) {
        return Ok(DecodeDecision::NeedsConfirmation {
            suggested_encoding: CharacterEncoding::Utf16Le,
            reason: EncodingChoiceReason::BomlessUtf16,
        });
    }
    if looks_like_utf16(bytes, false) {
        return Ok(DecodeDecision::NeedsConfirmation {
            suggested_encoding: CharacterEncoding::Utf16Be,
            reason: EncodingChoiceReason::BomlessUtf16,
        });
    }
    if looks_binary(bytes) {
        return Err("This appears to be a binary file, not a text document.".to_string());
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Ok(DecodeDecision::Decoded {
            text: text.to_owned(),
            encoding: CharacterEncoding::Utf8,
        });
    }
    if is_valid_windows_1252(bytes) {
        return Ok(DecodeDecision::NeedsConfirmation {
            suggested_encoding: CharacterEncoding::Windows1252,
            reason: EncodingChoiceReason::LegacySingleByte,
        });
    }
    Err("The file's character encoding is unsupported or invalid.".to_string())
}

pub fn decode_explicit(
    bytes: &[u8],
    encoding: CharacterEncoding,
) -> Result<DecodeDecision, String> {
    Ok(DecodeDecision::Decoded {
        text: decode_as(bytes, encoding)?,
        encoding,
    })
}

pub fn encode_strict(content: &str, encoding: CharacterEncoding) -> Result<Cow<'_, [u8]>, String> {
    match encoding {
        CharacterEncoding::Utf8 => Ok(Cow::Borrowed(content.as_bytes())),
        CharacterEncoding::Utf8Bom => {
            let mut bytes = Vec::with_capacity(UTF8_BOM.len() + content.len());
            bytes.extend_from_slice(UTF8_BOM);
            bytes.extend_from_slice(content.as_bytes());
            Ok(Cow::Owned(bytes))
        }
        CharacterEncoding::Utf16Le | CharacterEncoding::Utf16Be => {
            let little_endian = encoding == CharacterEncoding::Utf16Le;
            let mut bytes = Vec::with_capacity(2 + content.len() * 2);
            bytes.extend_from_slice(if little_endian {
                UTF16_LE_BOM
            } else {
                UTF16_BE_BOM
            });
            for unit in content.encode_utf16() {
                let encoded = if little_endian {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                };
                bytes.extend_from_slice(&encoded);
            }
            Ok(Cow::Owned(bytes))
        }
        CharacterEncoding::Windows1252 => {
            let mut encoder = WINDOWS_1252.new_encoder();
            let max_len = encoder
                .max_buffer_length_from_utf8_if_no_unmappables(content.len())
                .ok_or_else(|| "Text is too large to encode as Windows-1252.".to_string())?;
            let mut bytes = Vec::with_capacity(max_len);
            let (result, _) =
                encoder.encode_from_utf8_to_vec_without_replacement(content, &mut bytes, true);
            match result {
                EncoderResult::InputEmpty => Ok(Cow::Owned(bytes)),
                EncoderResult::Unmappable(character) => Err(format!(
                    "Character {character:?} cannot be represented in Windows-1252. Nothing was saved."
                )),
                EncoderResult::OutputFull => {
                    Err("Text is too large to encode as Windows-1252.".to_string())
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_boms_and_strips_them() {
        assert_eq!(
            detect_and_decode(b"\xEF\xBB\xBFhello").unwrap(),
            DecodeDecision::Decoded {
                text: "hello".into(),
                encoding: CharacterEncoding::Utf8Bom,
            }
        );
        assert_eq!(
            detect_and_decode(b"\xFF\xFEh\0i\0").unwrap(),
            DecodeDecision::Decoded {
                text: "hi".into(),
                encoding: CharacterEncoding::Utf16Le,
            }
        );
    }

    #[test]
    fn asks_before_legacy_or_bomless_utf16() {
        assert_eq!(
            detect_and_decode(b"caf\xE9").unwrap(),
            DecodeDecision::NeedsConfirmation {
                suggested_encoding: CharacterEncoding::Windows1252,
                reason: EncodingChoiceReason::LegacySingleByte,
            }
        );
        assert_eq!(
            detect_and_decode(b"h\0i\0").unwrap(),
            DecodeDecision::NeedsConfirmation {
                suggested_encoding: CharacterEncoding::Utf16Le,
                reason: EncodingChoiceReason::BomlessUtf16,
            }
        );
    }

    #[test]
    fn windows_1252_encoding_is_strict() {
        assert_eq!(
            decode_explicit(b"caf\xE9", CharacterEncoding::Windows1252).unwrap(),
            DecodeDecision::Decoded {
                text: "café".into(),
                encoding: CharacterEncoding::Windows1252,
            }
        );
        assert_eq!(
            encode_strict("café €", CharacterEncoding::Windows1252)
                .unwrap()
                .as_ref(),
            b"caf\xE9 \x80"
        );
        assert!(encode_strict("emoji 😀", CharacterEncoding::Windows1252).is_err());

        let valid_bytes: Vec<u8> = (0_u8..=u8::MAX)
            .filter(|byte| !matches!(byte, 0x81 | 0x8D | 0x8F | 0x90 | 0x9D))
            .collect();
        let decoded = match decode_explicit(&valid_bytes, CharacterEncoding::Windows1252).unwrap() {
            DecodeDecision::Decoded { text, .. } => text,
            DecodeDecision::NeedsConfirmation { .. } => unreachable!(),
        };
        assert_eq!(
            encode_strict(&decoded, CharacterEncoding::Windows1252)
                .unwrap()
                .as_ref(),
            valid_bytes
        );
    }

    #[test]
    fn supported_unicode_encodings_round_trip() {
        let text = "plain β 😀\n";
        for encoding in [
            CharacterEncoding::Utf8,
            CharacterEncoding::Utf8Bom,
            CharacterEncoding::Utf16Le,
            CharacterEncoding::Utf16Be,
        ] {
            let bytes = encode_strict(text, encoding).unwrap();
            assert_eq!(
                decode_explicit(bytes.as_ref(), encoding).unwrap(),
                DecodeDecision::Decoded {
                    text: text.into(),
                    encoding,
                }
            );
        }
    }

    #[test]
    fn rejects_malformed_or_mismatched_utf16() {
        assert!(decode_explicit(&[0xFF, 0xFE, b'a'], CharacterEncoding::Utf16Le).is_err());
        assert!(decode_explicit(&[0xFE, 0xFF, 0, b'a'], CharacterEncoding::Utf16Le).is_err());
        assert!(decode_explicit(&[0xFF, 0xFE, 0x00, 0xD8], CharacterEncoding::Utf16Le).is_err());
    }

    #[test]
    fn rejects_binary_and_utf32() {
        assert!(detect_and_decode(&[0, 0, 0xFE, 0xFF, 0, 0, 0, b'a']).is_err());
        assert!(detect_and_decode(&[1, 2, 0, 3, 4]).is_err());
        assert!(decode_explicit(&[0x81], CharacterEncoding::Windows1252).is_err());
    }
}
