//! Shrinks a TrueType font to the glyphs a document uses.
//!
//! Glyph ids and the character map are kept unchanged, so the PDF writer can keep
//! mapping text to glyphs as with the full font: unused outlines become empty, and
//! shaping tables (unused by PDF viewers) and glyph names are dropped.
use anyhow::{Context, Result, bail, ensure};
use std::collections::BTreeSet;

const DROPPED: [&[u8; 4]; 5] = [b"GSUB", b"GPOS", b"GDEF", b"DSIG", b"LTSH"];

fn u16_at(data: &[u8], at: usize) -> Result<u16> {
    let bytes = data.get(at..at + 2).context("Truncated font")?;
    Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn u32_at(data: &[u8], at: usize) -> Result<u32> {
    let bytes = data.get(at..at + 4).context("Truncated font")?;
    Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0u32, |sum, chunk| {
        let mut word = [0u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}

pub fn subset(font: &[u8], text: impl IntoIterator<Item = char>) -> Result<Vec<u8>> {
    let face = ttf_parser::Face::parse(font, 0).context("Invalid font")?;
    let mut keep: BTreeSet<u16> = BTreeSet::from([0]);
    for c in text {
        if let Some(id) = face.glyph_index(c) {
            keep.insert(id.0);
        }
    }

    let count = u16_at(font, 4)? as usize;
    let mut tables: Vec<([u8; 4], Vec<u8>)> = Vec::with_capacity(count);
    for index in 0..count {
        let record = 12 + 16 * index;
        let mut tag = [0u8; 4];
        tag.copy_from_slice(font.get(record..record + 4).context("Truncated font")?);
        let offset = u32_at(font, record + 8)? as usize;
        let length = u32_at(font, record + 12)? as usize;
        let data = font
            .get(offset..offset + length)
            .context("Truncated font")?;
        tables.push((tag, data.to_vec()));
    }
    let table = |tag: &[u8; 4]| {
        tables
            .iter()
            .find(|(t, _)| t == tag)
            .map(|(_, data)| data.as_slice())
            .with_context(|| format!("Missing {} table", String::from_utf8_lossy(tag)))
    };
    let head = table(b"head")?;
    let glyphs = u16_at(table(b"maxp")?, 4)? as usize;
    let long_loca = u16_at(head, 50)? == 1;
    let loca = table(b"loca")?;
    let glyf = table(b"glyf")?;
    let offsets = (0..=glyphs)
        .map(|i| {
            Ok(if long_loca {
                u32_at(loca, 4 * i)? as usize
            } else {
                u16_at(loca, 2 * i)? as usize * 2
            })
        })
        .collect::<Result<Vec<_>>>()?;

    // Composite glyphs reference other glyphs: keep those too.
    let mut pending: Vec<u16> = keep.iter().copied().collect();
    while let Some(id) = pending.pop() {
        let (start, end) = (offsets[id as usize], offsets[id as usize + 1]);
        if end <= start || (u16_at(glyf, start)? as i16) >= 0 {
            continue;
        }
        let mut at = start + 10;
        loop {
            let flags = u16_at(glyf, at)?;
            let component = u16_at(glyf, at + 2)?;
            ensure!((component as usize) < glyphs, "Invalid composite glyph");
            if keep.insert(component) {
                pending.push(component);
            }
            at += 4 + if flags & 0x0001 != 0 { 4 } else { 2 };
            at += if flags & 0x0008 != 0 {
                2
            } else if flags & 0x0040 != 0 {
                4
            } else if flags & 0x0080 != 0 {
                8
            } else {
                0
            };
            if flags & 0x0020 == 0 {
                break;
            }
        }
    }

    let mut new_glyf = Vec::new();
    let mut new_loca = Vec::with_capacity((glyphs + 1) * 4);
    for id in 0..glyphs {
        new_loca.extend_from_slice(&(new_glyf.len() as u32).to_be_bytes());
        if keep.contains(&(id as u16)) {
            let data = glyf
                .get(offsets[id]..offsets[id + 1])
                .context("Truncated glyph")?;
            new_glyf.extend_from_slice(data);
            while new_glyf.len() % 4 != 0 {
                new_glyf.push(0);
            }
        }
    }
    new_loca.extend_from_slice(&(new_glyf.len() as u32).to_be_bytes());

    tables.retain(|(tag, _)| !DROPPED.contains(&tag));
    for (tag, data) in &mut tables {
        match &*tag {
            b"glyf" => *data = std::mem::take(&mut new_glyf),
            b"loca" => *data = std::mem::take(&mut new_loca),
            b"head" => {
                data[50..52].copy_from_slice(&1u16.to_be_bytes());
                data[8..12].fill(0);
            }
            // Version 3: no glyph names.
            b"post" => {
                if data.len() < 32 {
                    bail!("Invalid post table");
                }
                data.truncate(32);
                data[..4].copy_from_slice(&0x0003_0000u32.to_be_bytes());
            }
            _ => {}
        }
    }
    tables.sort_by_key(|(tag, _)| *tag);

    let count = tables.len() as u16;
    let power = 1u16 << (15 - count.leading_zeros());
    let mut output = Vec::new();
    output.extend_from_slice(&u32_at(font, 0)?.to_be_bytes());
    for value in [
        count,
        power * 16,
        power.trailing_zeros() as u16,
        count * 16 - power * 16,
    ] {
        output.extend_from_slice(&value.to_be_bytes());
    }
    let mut offset = 12 + 16 * tables.len();
    let mut head_at = None;
    for (tag, data) in &tables {
        output.extend_from_slice(tag);
        output.extend_from_slice(&checksum(data).to_be_bytes());
        output.extend_from_slice(&(offset as u32).to_be_bytes());
        output.extend_from_slice(&(data.len() as u32).to_be_bytes());
        if tag == b"head" {
            head_at = Some(offset);
        }
        offset += data.len().div_ceil(4) * 4;
    }
    for (_, data) in &tables {
        output.extend_from_slice(data);
        while output.len() % 4 != 0 {
            output.push(0);
        }
    }
    let head_at = head_at.context("Missing head table")?;
    let adjustment = 0xB1B0_AFBAu32.wrapping_sub(checksum(&output));
    output[head_at + 8..head_at + 12].copy_from_slice(&adjustment.to_be_bytes());
    Ok(output)
}

#[cfg(test)]
mod tests {
    const FONT: &[u8] = include_bytes!("../../../assets/fonts/NotoSans-Regular.ttf");

    #[test]
    fn keeps_used_glyphs_and_their_ids() {
        let small = super::subset(FONT, "Société 6 000,00 €".chars()).unwrap();
        assert!(small.len() < FONT.len() / 10, "{} bytes", small.len());
        let full = ttf_parser::Face::parse(FONT, 0).unwrap();
        let face = ttf_parser::Face::parse(&small, 0).unwrap();
        assert_eq!(face.number_of_glyphs(), full.number_of_glyphs());
        let id = |face: &ttf_parser::Face<'_>, c| face.glyph_index(c).unwrap();
        assert_eq!(id(&face, 'é'), id(&full, 'é'));
        assert!(face.glyph_bounding_box(id(&face, 'é')).is_some());
        assert!(face.glyph_bounding_box(id(&face, 'Z')).is_none());
        assert_eq!(
            face.glyph_hor_advance(id(&face, 'Z')),
            full.glyph_hor_advance(id(&full, 'Z'))
        );
    }
}
