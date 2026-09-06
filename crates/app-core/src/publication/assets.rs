use super::{BookAsset, PublicationIssue};

pub(crate) fn asset_path(value: &str) -> Result<String, PublicationIssue> {
    let value = value
        .strip_prefix("../assets/")
        .map(|v| format!("assets/{v}"))
        .unwrap_or_else(|| value.to_owned());
    if value.len() > 512
        || !value.starts_with("assets/")
        || value.contains(['\\', ':', '?', '#', '%'])
        || value.chars().any(char::is_control)
        || value.split('/').any(|p| {
            p.is_empty()
                || p.starts_with('.')
                || p.ends_with(['.', ' '])
                || p.trim() != p
                || reserved(p)
        })
    {
        return Err(PublicationIssue::UnsafePath);
    }
    let extension = value.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    if !matches!(extension.as_str(), "png" | "jpg" | "jpeg") {
        return Err(PublicationIssue::InvalidAsset);
    }
    Ok(value)
}

pub(crate) fn reserved(part: &str) -> bool {
    let base = part.split('.').next().unwrap_or("").to_ascii_uppercase();
    matches!(
        base.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].into_iter().any(|prefix| {
        base.strip_prefix(prefix)
            .is_some_and(|n| matches!(n, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"))
    })
}

fn dimensions(width: u32, height: u32) -> Result<(), PublicationIssue> {
    if width == 0
        || height == 0
        || width > 8192
        || height > 8192
        || u64::from(width) * u64::from(height) > 16_777_216
    {
        return Err(PublicationIssue::InvalidAsset);
    }
    Ok(())
}

pub(crate) fn validate(path: &str, bytes: Vec<u8>) -> Result<BookAsset, PublicationIssue> {
    let safe = asset_path(path)?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(PublicationIssue::LimitExceeded);
    }
    let png = safe.to_ascii_lowercase().ends_with(".png");
    if png {
        validate_png(&bytes)?;
    } else {
        validate_jpeg(&bytes)?;
    }
    Ok(BookAsset {
        bytes,
        media_type: if png { "image/png" } else { "image/jpeg" },
        extension: if png { "png" } else { "jpg" },
    })
}

fn validate_png(bytes: &[u8]) -> Result<(), PublicationIssue> {
    let bad = PublicationIssue::InvalidAsset;
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(bad);
    }
    let mut offset = 8;
    let mut header = false;
    let mut data = false;
    let mut data_ended = false;
    let mut palette = false;
    let mut indexed = false;
    while offset + 12 <= bytes.len() {
        let length =
            u32::from_be_bytes(bytes[offset..offset + 4].try_into().map_err(|_| bad)?) as usize;
        let end = offset
            .checked_add(12 + length)
            .filter(|end| *end <= bytes.len())
            .ok_or(bad)?;
        let kind = &bytes[offset + 4..offset + 8];
        let content = &bytes[offset + 8..end - 4];
        let crc = u32::from_be_bytes(bytes[end - 4..end].try_into().map_err(|_| bad)?);
        if crc32fast::hash(&bytes[offset + 4..end - 4]) != crc {
            return Err(bad);
        }
        if !header && kind != b"IHDR" {
            return Err(bad);
        }
        match kind {
            b"IHDR" => {
                if header || length != 13 {
                    return Err(bad);
                }
                dimensions(
                    u32::from_be_bytes(content[0..4].try_into().map_err(|_| bad)?),
                    u32::from_be_bytes(content[4..8].try_into().map_err(|_| bad)?),
                )?;
                let valid_depth = match content[9] {
                    0 => matches!(content[8], 1 | 2 | 4 | 8 | 16),
                    2 | 4 | 6 => matches!(content[8], 8 | 16),
                    3 => matches!(content[8], 1 | 2 | 4 | 8),
                    _ => false,
                };
                if !valid_depth || content[10] != 0 || content[11] != 0 || content[12] > 1 {
                    return Err(bad);
                }
                indexed = content[9] == 3;
                header = true;
            }
            b"PLTE" => {
                if data || palette || length == 0 || length > 768 || !length.is_multiple_of(3) {
                    return Err(bad);
                }
                palette = true;
            }
            b"IDAT" => {
                if data_ended || indexed && !palette {
                    return Err(bad);
                }
                data = true;
            }
            b"IEND" => {
                return if data && length == 0 && end == bytes.len() {
                    Ok(())
                } else {
                    Err(bad)
                }
            }
            b"acTL" | b"fcTL" | b"fdAT" => return Err(bad),
            _ => {
                if kind.iter().any(|b| !b.is_ascii_alphabetic()) || kind[0].is_ascii_uppercase() {
                    return Err(bad);
                }
                if data {
                    data_ended = true;
                }
            }
        }
        offset = end;
    }
    Err(bad)
}

// Bounded JPEG marker/dimension consistency; not a pixel decoder or integrity certification.
fn validate_jpeg(bytes: &[u8]) -> Result<(), PublicationIssue> {
    let bad = PublicationIssue::InvalidAsset;
    if !bytes.starts_with(&[0xff, 0xd8]) || !bytes.ends_with(&[0xff, 0xd9]) {
        return Err(bad);
    }
    let mut offset = 2;
    let mut frame = false;
    let mut scan = false;
    let mut entropy = false;
    while offset < bytes.len() {
        if entropy {
            while offset < bytes.len() && bytes[offset] != 0xff {
                offset += 1;
            }
        }
        if bytes.get(offset) != Some(&0xff) {
            return Err(bad);
        }
        while bytes.get(offset) == Some(&0xff) {
            offset += 1;
        }
        let marker = *bytes.get(offset).ok_or(bad)?;
        offset += 1;
        if entropy && (marker == 0 || (0xd0..=0xd7).contains(&marker)) {
            continue;
        }
        entropy = false;
        if marker == 0xd9 {
            return if frame && scan && offset == bytes.len() {
                Ok(())
            } else {
                Err(bad)
            };
        }
        if matches!(marker, 0 | 0x01 | 0xd0..=0xd8) {
            return Err(bad);
        }
        let length = u16::from_be_bytes(
            bytes
                .get(offset..offset + 2)
                .ok_or(bad)?
                .try_into()
                .map_err(|_| bad)?,
        ) as usize;
        if length < 2 {
            return Err(bad);
        }
        let end = offset
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or(bad)?;
        let content = &bytes[offset + 2..end];
        if matches!(marker, 0xc0 | 0xc2) {
            if frame
                || content.len() < 6
                || content[0] != 8
                || !matches!(content[5], 1 | 3 | 4)
                || content.len() != 6 + 3 * usize::from(content[5])
            {
                return Err(bad);
            }
            dimensions(
                u32::from(u16::from_be_bytes([content[3], content[4]])),
                u32::from(u16::from_be_bytes([content[1], content[2]])),
            )?;
            frame = true;
        } else if matches!(marker, 0xc1 | 0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf) {
            return Err(bad);
        } else if marker == 0xda {
            if !frame
                || content.len() < 4
                || content[0] == 0
                || content[0] > 4
                || content.len() != 4 + 2 * usize::from(content[0])
            {
                return Err(bad);
            }
            scan = true;
            entropy = true;
        } else if !matches!(marker, 0xc4 | 0xdb | 0xdd | 0xe0..=0xef | 0xfe) {
            return Err(bad);
        }
        offset = end;
    }
    Err(bad)
}
