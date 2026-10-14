//! Bounded format framing preflight, without decompressing pixels.
//! In particular, absence of a loop extension means one play, not infinite looping.
pub(super) struct GifMetadata {
    pub loops: u32,
    pub delays: Vec<u16>,
}

fn take<'a>(bytes: &'a [u8], offset: &mut usize, size: usize) -> Result<&'a [u8], String> {
    let end = offset
        .checked_add(size)
        .ok_or("Image block length overflow")?;
    let value = bytes.get(*offset..end).ok_or("Truncated image block")?;
    *offset = end;
    Ok(value)
}

fn metadata(total: &mut usize, length: usize) -> Result<(), String> {
    *total = total.checked_add(length).ok_or("Image metadata overflow")?;
    if *total > super::MAX_METADATA {
        return Err("Image metadata exceeds 1 MiB".into());
    }
    Ok(())
}

fn canvas_frames(width: u32, height: u32, frames: u32) -> Result<(), String> {
    let pixels = super::dimensions(width, height)?;
    if frames == 0 || frames > super::MAX_FRAMES || pixels * u64::from(frames) > super::MAX_PIXELS {
        return Err("Animation exceeds frame/cumulative decoded-pixel limits".into());
    }
    Ok(())
}

fn be32(bytes: &[u8]) -> Result<u32, String> {
    Ok(u32::from_be_bytes(
        bytes.try_into().map_err(|_| "Invalid PNG integer")?,
    ))
}

fn le32(bytes: &[u8]) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        bytes.try_into().map_err(|_| "Invalid WebP integer")?,
    ))
}

pub(super) fn png(bytes: &[u8]) -> Result<(), String> {
    let mut offset = 8;
    let mut total = 0;
    let mut dimensions = None;
    let mut declared = None;
    let mut frames = 0u32;
    let mut separate_default = false;
    loop {
        let length = be32(take(bytes, &mut offset, 4)?)? as usize;
        let tag = take(bytes, &mut offset, 4)?;
        let data = take(bytes, &mut offset, length)?;
        take(bytes, &mut offset, 4)?; // CRC is checked by the maintained decoder.
        if !matches!(tag, b"IDAT" | b"fdAT") {
            metadata(&mut total, length)?;
        }
        match tag {
            b"IHDR" if data.len() == 13 && dimensions.is_none() => {
                let width = be32(&data[..4])?;
                let height = be32(&data[4..8])?;
                super::dimensions(width, height)?;
                dimensions = Some((width, height));
            }
            b"acTL" if data.len() == 8 && declared.is_none() => {
                declared = Some(be32(&data[..4])?);
            }
            b"fcTL" => {
                frames = frames.checked_add(1).ok_or("PNG frame overflow")?;
                if frames > super::MAX_FRAMES {
                    return Err("PNG exceeds 1000 frames".into());
                }
            }
            b"IDAT" if declared.is_some() && frames == 0 => separate_default = true,
            b"IEND" => {
                if length != 0 || offset != bytes.len() {
                    return Err("Invalid PNG ending".into());
                }
                let (width, height) = dimensions.ok_or("Missing PNG dimensions")?;
                if let Some(declared) = declared {
                    if frames != declared {
                        return Err("PNG frame count differs from animation control".into());
                    }
                    canvas_frames(width, height, frames)?;
                    // A separate default still costs decoded pixels, even though
                    // it must not appear in the animation timeline.
                    if separate_default
                        && u64::from(width) * u64::from(height) * u64::from(frames + 1)
                            > super::MAX_PIXELS
                    {
                        return Err("PNG default image exceeds decoded-pixel budget".into());
                    }
                } else if frames != 0 {
                    return Err("PNG frame control without animation control".into());
                }
                return Ok(());
            }
            _ => {}
        }
    }
}

pub(super) fn webp(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() < 12 || u64::from(le32(&bytes[4..8])?) + 8 != bytes.len() as u64 {
        return Err("Invalid WebP RIFF length".into());
    }
    let mut offset = 12;
    let mut total = 0;
    let mut dimensions = None;
    let mut frames = 0;
    while offset < bytes.len() {
        let tag = take(bytes, &mut offset, 4)?;
        let length = le32(take(bytes, &mut offset, 4)?)? as usize;
        let data = take(bytes, &mut offset, length)?;
        if !length.is_multiple_of(2) {
            take(bytes, &mut offset, 1)?;
        }
        match tag {
            b"VP8X" if data.len() == 10 => {
                let dimension = |data: &[u8]| {
                    1 + u32::from(data[0]) + (u32::from(data[1]) << 8) + (u32::from(data[2]) << 16)
                };
                let width = dimension(&data[4..7]);
                let height = dimension(&data[7..10]);
                super::dimensions(width, height)?;
                dimensions = Some((width, height));
            }
            b"ANMF" => {
                frames += 1;
                let (width, height) = dimensions.ok_or("WebP animation lacks canvas dimensions")?;
                canvas_frames(width, height, frames)?;
                if data.len() < 16 {
                    return Err("Truncated WebP frame header".into());
                }
            }
            b"VP8 " | b"VP8L" | b"ALPH" => {}
            _ => metadata(&mut total, length)?,
        }
    }
    Ok(())
}

pub(super) fn jpeg(bytes: &[u8]) -> Result<(), String> {
    let mut offset = 2;
    let mut total = 0;
    let mut entropy = false;
    while offset < bytes.len() {
        let next = take(bytes, &mut offset, 1)?[0];
        if next != 0xff {
            if entropy {
                continue;
            }
            return Err("Invalid JPEG marker".into());
        }
        let mut marker = take(bytes, &mut offset, 1)?[0];
        while marker == 0xff {
            marker = take(bytes, &mut offset, 1)?[0];
        }
        if entropy && matches!(marker, 0 | 0xd0..=0xd7) {
            continue;
        }
        if marker == 0xd9 {
            return Ok(());
        }
        if marker == 0x01 {
            continue;
        }
        let length = take(bytes, &mut offset, 2)?;
        let size = usize::from(u16::from_be_bytes([length[0], length[1]]));
        let data = take(
            bytes,
            &mut offset,
            size.checked_sub(2).ok_or("Invalid JPEG segment length")?,
        )?;
        metadata(&mut total, data.len())?;
        if matches!(marker, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf)
            && data.first() != Some(&8)
        {
            return Err("Unsupported JPEG sample precision; expected 8-bit integer samples".into());
        }
        entropy = marker == 0xda;
    }
    Err("JPEG is missing its end marker".into())
}

fn blocks(
    bytes: &[u8],
    offset: &mut usize,
    metadata: bool,
    total: &mut usize,
) -> Result<Vec<u8>, String> {
    let mut value = Vec::new();
    loop {
        let length = usize::from(take(bytes, offset, 1)?[0]);
        if length == 0 {
            return Ok(value);
        }
        let block = take(bytes, offset, length)?;
        if metadata {
            *total = total
                .checked_add(length)
                .ok_or("GIF metadata length overflow")?;
            if *total > super::MAX_METADATA {
                return Err("GIF metadata exceeds 1 MiB".into());
            }
            value.extend_from_slice(block);
        }
    }
}

pub(super) fn gif(bytes: &[u8]) -> Result<GifMetadata, String> {
    let mut offset = 0;
    let header = take(bytes, &mut offset, 13)?;
    if !matches!(&header[..6], b"GIF87a" | b"GIF89a") {
        return Err("Invalid GIF signature".into());
    }
    super::dimensions(
        u32::from(u16::from_le_bytes([header[6], header[7]])),
        u32::from(u16::from_le_bytes([header[8], header[9]])),
    )?;
    if header[10] & 0x80 != 0 {
        take(bytes, &mut offset, 3usize << ((header[10] & 7) + 1))?;
    }
    let mut result = GifMetadata {
        loops: 1,
        delays: Vec::new(),
    };
    let mut delay = 0;
    let mut metadata = 0;
    loop {
        match take(bytes, &mut offset, 1)?[0] {
            0x3b => {
                if result.delays.is_empty() {
                    return Err("GIF contains no frames".into());
                }
                canvas_frames(
                    u32::from(u16::from_le_bytes([header[6], header[7]])),
                    u32::from(u16::from_le_bytes([header[8], header[9]])),
                    u32::try_from(result.delays.len()).map_err(|_| "GIF frame overflow")?,
                )?;
                return Ok(result);
            }
            0x21 => {
                let label = take(bytes, &mut offset, 1)?[0];
                let extension = blocks(bytes, &mut offset, true, &mut metadata)?;
                if label == 0x01 {
                    return Err("Unsupported GIF plain-text rendering block".into());
                }
                if label == 0xf9 {
                    if extension.len() != 4 {
                        return Err("Invalid GIF control extension".into());
                    }
                    if extension[0] & 2 != 0 {
                        return Err(
                            "GIF user-input timing cannot be represented exactly in JPEG XL".into(),
                        );
                    }
                    if (extension[0] >> 2) & 7 > 3 {
                        return Err("Unsupported GIF disposal method".into());
                    }
                    delay = u16::from_le_bytes([extension[1], extension[2]]);
                } else if label == 0xff
                    && extension.len() >= 14
                    && matches!(&extension[..11], b"NETSCAPE2.0" | b"ANIMEXTS1.0")
                {
                    if extension[11] != 1 {
                        return Err("Invalid GIF loop extension".into());
                    }
                    let repeats = u16::from_le_bytes([extension[12], extension[13]]);
                    result.loops = if repeats == 0 {
                        0
                    } else {
                        u32::from(repeats) + 1
                    };
                }
            }
            0x2c => {
                let frame = take(bytes, &mut offset, 9)?;
                if result.delays.len() >= super::MAX_FRAMES as usize {
                    return Err("GIF exceeds 1000 frames".into());
                }
                result.delays.push(delay);
                delay = 0;
                if frame[8] & 0x80 != 0 {
                    take(bytes, &mut offset, 3usize << ((frame[8] & 7) + 1))?;
                }
                take(bytes, &mut offset, 1)?; // LZW minimum code size; decoder validates it.
                blocks(bytes, &mut offset, false, &mut metadata)?;
            }
            _ => return Err("Invalid GIF block".into()),
        }
    }
}
