//! A livery's preview picture, turned into something a browser can draw.
//!
//! A livery mod ships each car's preview beside its body textures — a wide render of the car on
//! a transparent background, declared as `<PREVIEWIMAGE PATH="...">` in the manifest
//! [`crate::liveries`] already reads. It is a `.dds`: a block-compressed texture no browser will
//! display, and an image crate to decode it would pull in more dependencies than the rest of this
//! program has together. So it is decoded here and re-encoded as a PNG thumbnail.
//!
//! Only the block formats a livery mod actually ships are decoded (BC1/BC2/BC3, i.e.
//! `DXT1`/`DXT3`/`DXT5`) plus plain uncompressed pixels. Anything else — notably BC7, which needs
//! a decoder several times the size of this file — is reported as unsupported rather than guessed
//! at, and the tab simply shows no picture for that car.
//!
//! **The thumbnail is produced in one pass, never as a full-size image.** A 2048×768 preview is
//! 6 MB decoded and would be built only to be thrown away, since the source is scaled down by a
//! factor of six on the way out. Each block's pixels are therefore accumulated straight into the
//! target's bins, so the largest buffer here is the thumbnail itself.

/// Width of the thumbnail the Car Performance tab shows, in pixels. Twice its display width, so
/// it stays sharp on a hidpi screen; the height follows the source's aspect ratio.
pub const THUMB_WIDTH: u32 = 320;

/// Decodes a `.dds` and re-encodes it as a PNG no wider than `width`.
///
/// The image is never enlarged: a preview already narrower than `width` is returned at its own
/// size rather than scaled up into a blur.
pub fn dds_thumbnail_png(dds: &[u8], width: u32) -> Result<Vec<u8>, String> {
    let (w, h, pixels) = decode_scaled(dds, width)?;
    Ok(encode_png(w, h, &pixels))
}

// ── DDS ──────────────────────────────────────────────────────────────────────

/// How a DDS stores its pixels. The premultiplied-alpha variants (`DXT2`, `DXT4`) have the same
/// layout as the ones they pair with and are decoded as those — the difference is in how the
/// colour is meant to be blended, which a thumbnail never notices.
enum Format {
    /// BC1: one colour block per 4×4, alpha only as a one-bit hole.
    Bc1,
    /// BC2: BC1 colour with four-bit explicit alpha.
    Bc2,
    /// BC3: BC1 colour with interpolated alpha.
    Bc3,
    /// Plain pixels, `bytes_per_pixel` each, channels located by mask.
    Raw {
        bytes_per_pixel: usize,
        masks: [u32; 4],
    },
}

fn le32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// Width, height, pixel format, and the offset the pixel data starts at.
fn parse_header(dds: &[u8]) -> Result<(u32, u32, Format, usize), String> {
    if dds.len() < 128 || &dds[0..4] != b"DDS " || le32(dds, 4) != 124 {
        return Err("not a DDS file".into());
    }
    let height = le32(dds, 12);
    let width = le32(dds, 16);
    if width == 0 || height == 0 {
        return Err("DDS declares no pixels".into());
    }
    let flags = le32(dds, 80);
    // DDPF_FOURCC — a named, usually block-compressed, format.
    if flags & 0x4 != 0 {
        let format = match &dds[84..88] {
            b"DXT1" => Format::Bc1,
            b"DXT2" | b"DXT3" => Format::Bc2,
            b"DXT4" | b"DXT5" => Format::Bc3,
            other => {
                let name = String::from_utf8_lossy(other).trim().to_string();
                return Err(format!("unsupported DDS format {name}"));
            }
        };
        return Ok((width, height, format, 128));
    }
    // Uncompressed: the channels are wherever their masks say they are.
    let bits = le32(dds, 88);
    if bits == 0 || !bits.is_multiple_of(8) || bits > 32 {
        return Err(format!("unsupported DDS pixel size ({bits} bits)"));
    }
    let masks = [le32(dds, 92), le32(dds, 96), le32(dds, 100), le32(dds, 104)];
    Ok((
        width,
        height,
        Format::Raw {
            bytes_per_pixel: (bits / 8) as usize,
            masks,
        },
        128,
    ))
}

/// A 4-, 5-, 6- or 8-bit channel widened to the full 0..=255 range. Plain shifting would leave
/// white at 248 rather than 255, which shows up as a grey cast over the whole picture.
fn widen(value: u32, bits: u32) -> u8 {
    if bits == 0 {
        return 255;
    }
    let max = (1u32 << bits) - 1;
    ((value * 255 + max / 2) / max) as u8
}

fn rgb565(c: u16) -> [u8; 3] {
    [
        widen((c as u32 >> 11) & 0x1F, 5),
        widen((c as u32 >> 5) & 0x3F, 6),
        widen(c as u32 & 0x1F, 5),
    ]
}

/// The four colours a BC1 colour block interpolates between.
///
/// `punch_through` is only true for BC1: in BC2 and BC3 the same `c0 <= c1` pattern still means
/// the four-colour table, because those formats carry their alpha separately.
fn colour_table(block: &[u8], punch_through: bool) -> [[u8; 3]; 4] {
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);
    let (a, b) = (rgb565(c0), rgb565(c1));
    let mut table = [[0u8; 3]; 4];
    table[0] = a;
    table[1] = b;
    for i in 0..3 {
        let (x, y) = (a[i] as u32, b[i] as u32);
        if punch_through && c0 <= c1 {
            table[2][i] = ((x + y) / 2) as u8;
            table[3][i] = 0;
        } else {
            table[2][i] = ((2 * x + y) / 3) as u8;
            table[3][i] = ((x + 2 * y) / 3) as u8;
        }
    }
    table
}

/// The eight alpha values a BC3 alpha block interpolates between.
fn alpha_table(block: &[u8]) -> [u8; 8] {
    let (a0, a1) = (block[0] as u32, block[1] as u32);
    let mut table = [0u8; 8];
    table[0] = a0 as u8;
    table[1] = a1 as u8;
    if a0 > a1 {
        for (i, slot) in table.iter_mut().enumerate().skip(2) {
            *slot = (((8 - i as u32) * a0 + (i as u32 - 1) * a1) / 7) as u8;
        }
    } else {
        for (i, slot) in table.iter_mut().enumerate().take(6).skip(2) {
            *slot = (((6 - i as u32) * a0 + (i as u32 - 1) * a1) / 5) as u8;
        }
        table[6] = 0;
        table[7] = 255;
    }
    table
}

/// One target pixel being averaged. Colour is weighted by alpha: a preview is a car on a fully
/// transparent background whose hidden RGB is arbitrary (usually black), so an unweighted average
/// would draw a dark halo round every edge of the car.
#[derive(Clone, Copy, Default)]
struct Bin {
    weighted: [u64; 3],
    alpha: u64,
    count: u32,
}

impl Bin {
    fn add(&mut self, rgb: [u8; 3], a: u8) {
        for (slot, channel) in self.weighted.iter_mut().zip(rgb) {
            *slot += channel as u64 * a as u64;
        }
        self.alpha += a as u64;
        self.count += 1;
    }

    /// A bin nothing reached, or one that is wholly transparent, is transparent black — there is
    /// no colour to recover from pixels that contribute none.
    fn resolve(&self) -> [u8; 4] {
        if self.count == 0 || self.alpha == 0 {
            return [0, 0, 0, 0];
        }
        [
            (self.weighted[0] / self.alpha) as u8,
            (self.weighted[1] / self.alpha) as u8,
            (self.weighted[2] / self.alpha) as u8,
            (self.alpha / self.count as u64) as u8,
        ]
    }
}

/// Decodes `dds` straight into a thumbnail: width, height and RGBA bytes.
///
/// Every source pixel falls into exactly one target bin, which is a box filter — the right choice
/// when shrinking by a large factor, and the reason no full-size buffer is needed.
fn decode_scaled(dds: &[u8], max_width: u32) -> Result<(u32, u32, Vec<u8>), String> {
    let (w, h, format, mut at) = parse_header(dds)?;
    let out_w = max_width.clamp(1, w);
    let out_h = (((h as u64 * out_w as u64) / w as u64).max(1)) as u32;
    let mut bins = vec![Bin::default(); (out_w as usize) * (out_h as usize)];

    let mut put = |x: u32, y: u32, rgb: [u8; 3], a: u8| {
        if x >= w || y >= h {
            return;
        }
        let tx = (((x as u64 * out_w as u64) / w as u64) as u32).min(out_w - 1);
        let ty = (((y as u64 * out_h as u64) / h as u64) as u32).min(out_h - 1);
        bins[(ty * out_w + tx) as usize].add(rgb, a);
    };

    match format {
        Format::Raw {
            bytes_per_pixel,
            masks,
        } => {
            let channels: Vec<(u32, u32)> = masks.iter().map(|&m| mask_shift(m)).collect();
            let need = bytes_per_pixel * w as usize * h as usize;
            if dds.len() < at + need {
                return Err("DDS is shorter than its own dimensions".into());
            }
            for y in 0..h {
                for x in 0..w {
                    let mut raw = 0u32;
                    for (i, byte) in dds[at..at + bytes_per_pixel].iter().enumerate() {
                        raw |= (*byte as u32) << (8 * i);
                    }
                    at += bytes_per_pixel;
                    let channel = |c: usize| -> u8 {
                        let (shift, bits) = channels[c];
                        if bits == 0 {
                            // No mask for this channel: opaque for alpha, black for colour.
                            return if c == 3 { 255 } else { 0 };
                        }
                        widen((raw >> shift) & ((1u32 << bits) - 1), bits)
                    };
                    put(
                        x,
                        y,
                        [channel(0), channel(1), channel(2)],
                        channel(3),
                    );
                }
            }
        }
        _ => {
            let (block_bytes, colour_at) = match format {
                Format::Bc1 => (8usize, 0usize),
                _ => (16, 8),
            };
            let blocks_x = w.div_ceil(4);
            let blocks_y = h.div_ceil(4);
            let need = block_bytes * blocks_x as usize * blocks_y as usize;
            if dds.len() < at + need {
                return Err("DDS is shorter than its own dimensions".into());
            }
            for by in 0..blocks_y {
                for bx in 0..blocks_x {
                    let block = &dds[at..at + block_bytes];
                    at += block_bytes;
                    let colour = &block[colour_at..];
                    let punch_through = matches!(format, Format::Bc1)
                        && u16::from_le_bytes([colour[0], colour[1]])
                            <= u16::from_le_bytes([colour[2], colour[3]]);
                    let colours = colour_table(colour, matches!(format, Format::Bc1));
                    let indices = le32(colour, 4);
                    let alphas = matches!(format, Format::Bc3).then(|| alpha_table(block));
                    for i in 0..16u32 {
                        let index = ((indices >> (2 * i)) & 3) as usize;
                        let alpha = match format {
                            // The fourth entry of a punch-through table is the hole, not a colour.
                            Format::Bc1 if punch_through && index == 3 => 0,
                            Format::Bc1 => 255,
                            Format::Bc2 => {
                                widen(((block[(i / 2) as usize] >> (4 * (i % 2))) & 0xF) as u32, 4)
                            }
                            _ => {
                                let packed = u64::from_le_bytes([
                                    block[2], block[3], block[4], block[5], block[6], block[7], 0,
                                    0,
                                ]);
                                alphas.unwrap()[((packed >> (3 * i)) & 7) as usize]
                            }
                        };
                        put(bx * 4 + i % 4, by * 4 + i / 4, colours[index], alpha);
                    }
                }
            }
        }
    }

    let mut pixels = Vec::with_capacity(bins.len() * 4);
    for bin in &bins {
        pixels.extend_from_slice(&bin.resolve());
    }
    Ok((out_w, out_h, pixels))
}

/// Where a channel's mask starts and how wide it is. `(0, 0)` for an absent channel.
fn mask_shift(mask: u32) -> (u32, u32) {
    if mask == 0 {
        return (0, 0);
    }
    let shift = mask.trailing_zeros();
    (shift, (mask >> shift).count_ones())
}

// ── PNG ──────────────────────────────────────────────────────────────────────

/// Encodes RGBA bytes as an eight-bit truecolour-with-alpha PNG.
fn encode_png(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let raw = filter_scanlines(w as usize, h as usize, rgba);
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    // Eight bits per channel, colour type 6 (RGBA), deflate, adaptive filtering, no interlace.
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &zlib(&raw));
    chunk(&mut out, b"IEND", &[]);
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// PNG's five per-row filters, each row taking whichever leaves the smallest total deviation from
/// zero — the heuristic the format's own specification recommends. Filtering is most of why a
/// rendered image compresses at all: it turns a smooth gradient into a run of near-zero bytes.
fn filter_scanlines(w: usize, h: usize, rgba: &[u8]) -> Vec<u8> {
    const BPP: usize = 4;
    let stride = w * BPP;
    let mut out = Vec::with_capacity(h * (stride + 1));
    let mut candidate = vec![0u8; stride];
    let mut chosen = vec![0u8; stride];
    for y in 0..h {
        let row = &rgba[y * stride..(y + 1) * stride];
        let prev: &[u8] = if y == 0 {
            &[]
        } else {
            &rgba[(y - 1) * stride..y * stride]
        };
        let up = |i: usize| if prev.is_empty() { 0u8 } else { prev[i] };
        let left = |i: usize| if i < BPP { 0u8 } else { row[i - BPP] };
        let up_left = |i: usize| {
            if i < BPP || prev.is_empty() {
                0u8
            } else {
                prev[i - BPP]
            }
        };
        let mut best_filter = 0u8;
        let mut best_score = u64::MAX;
        for filter in 0..5u8 {
            let mut score = 0u64;
            for (i, slot) in candidate.iter_mut().enumerate() {
                *slot = match filter {
                    0 => row[i],
                    1 => row[i].wrapping_sub(left(i)),
                    2 => row[i].wrapping_sub(up(i)),
                    3 => row[i].wrapping_sub(((left(i) as u16 + up(i) as u16) / 2) as u8),
                    _ => row[i].wrapping_sub(paeth(left(i), up(i), up_left(i))),
                };
                // Distance from zero counting 255 as −1: what is wanted is bytes a compressor
                // will find repetitive, not small unsigned numbers.
                score += (*slot as i8).unsigned_abs() as u64;
            }
            if score < best_score {
                best_score = score;
                best_filter = filter;
                chosen.copy_from_slice(&candidate);
            }
        }
        out.push(best_filter);
        out.extend_from_slice(&chosen);
    }
    out
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i16 + b as i16 - c as i16;
    let (pa, pb, pc) = (
        (p - a as i16).abs(),
        (p - b as i16).abs(),
        (p - c as i16).abs(),
    );
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                0xEDB8_8320 ^ (crc >> 1)
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn zlib(data: &[u8]) -> Vec<u8> {
    // 0x78 0x01: deflate, 32 KiB window, fastest compression setting.
    let mut out = vec![0x78, 0x01];
    out.extend_from_slice(&deflate_fixed(data));
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

// ── deflate ──────────────────────────────────────────────────────────────────

/// Bits go out least-significant first, except a Huffman code, whose bits go out
/// most-significant first. That asymmetry is deflate's, not this writer's.
struct BitWriter {
    out: Vec<u8>,
    hold: u32,
    bits: u32,
}

impl BitWriter {
    fn new() -> Self {
        BitWriter {
            out: Vec::new(),
            hold: 0,
            bits: 0,
        }
    }

    fn write(&mut self, value: u32, count: u32) {
        self.hold |= value << self.bits;
        self.bits += count;
        while self.bits >= 8 {
            self.out.push(self.hold as u8);
            self.hold >>= 8;
            self.bits -= 8;
        }
    }

    fn write_code(&mut self, code: u32, count: u32) {
        for i in (0..count).rev() {
            self.write((code >> i) & 1, 1);
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.bits > 0 {
            self.out.push(self.hold as u8);
        }
        self.out
    }
}

/// The fixed literal/length code of RFC 1951 §3.2.6, as (code, bit count).
fn fixed_literal(symbol: u16) -> (u32, u32) {
    match symbol {
        0..=143 => (0x30 + symbol as u32, 8),
        144..=255 => (0x190 + symbol as u32 - 144, 9),
        256..=279 => (symbol as u32 - 256, 7),
        _ => (0xC0 + symbol as u32 - 280, 8),
    }
}

const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u32; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DISTANCE_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DISTANCE_EXTRA: [u32; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;
const WINDOW: usize = 32768;
const HASH_BITS: u32 = 15;
/// How many earlier positions starting with the same three bytes are tried before settling for
/// the best match found so far. A thumbnail's long exact runs are found on the first try; the cap
/// is what stops a pathological input making this quadratic.
const MAX_CHAIN: usize = 32;

fn hash3(window: &[u8]) -> usize {
    let h = ((window[0] as u32) << 10) ^ ((window[1] as u32) << 5) ^ window[2] as u32;
    (h & ((1 << HASH_BITS) - 1)) as usize
}

/// One deflate block using the fixed Huffman codes, with greedy LZ77 matching.
///
/// Fixed codes rather than dynamic ones: a dynamic block would save perhaps a further tenth on
/// images this size, at the cost of a code-length tree, a second pass and the bookkeeping for
/// both. What actually shrinks a livery preview is the matching — the transparent background
/// either side of the car is one enormous run.
fn deflate_fixed(data: &[u8]) -> Vec<u8> {
    let mut w = BitWriter::new();
    w.write(1, 1); // final block
    w.write(1, 2); // fixed Huffman

    let mut head = vec![usize::MAX; 1 << HASH_BITS];
    let mut prev = vec![usize::MAX; data.len()];
    let mut at = 0usize;

    while at < data.len() {
        let mut best_len = 0usize;
        let mut best_dist = 0usize;
        if at + MIN_MATCH <= data.len() {
            let slot = hash3(&data[at..]);
            let mut candidate = head[slot];
            let mut tries = 0;
            while candidate != usize::MAX && tries < MAX_CHAIN && at - candidate <= WINDOW {
                tries += 1;
                let limit = MAX_MATCH.min(data.len() - at);
                let mut len = 0;
                while len < limit && data[candidate + len] == data[at + len] {
                    len += 1;
                }
                if len > best_len {
                    best_len = len;
                    best_dist = at - candidate;
                    if len == limit {
                        break;
                    }
                }
                candidate = prev[candidate];
            }
            prev[at] = head[slot];
            head[slot] = at;
        }

        if best_len < MIN_MATCH {
            let (code, bits) = fixed_literal(data[at] as u16);
            w.write_code(code, bits);
            at += 1;
            continue;
        }

        let li = LENGTH_BASE
            .iter()
            .rposition(|&b| b as usize <= best_len)
            .unwrap_or(0);
        let (code, bits) = fixed_literal(257 + li as u16);
        w.write_code(code, bits);
        w.write(best_len as u32 - LENGTH_BASE[li] as u32, LENGTH_EXTRA[li]);
        let di = DISTANCE_BASE
            .iter()
            .rposition(|&b| b as usize <= best_dist)
            .unwrap_or(0);
        w.write_code(di as u32, 5);
        w.write(best_dist as u32 - DISTANCE_BASE[di] as u32, DISTANCE_EXTRA[di]);

        // Every position the match covers still has to enter the table, or the next match has
        // nothing to chain back to.
        for i in at + 1..at + best_len {
            if i + MIN_MATCH <= data.len() {
                let slot = hash3(&data[i..]);
                prev[i] = head[slot];
                head[slot] = i;
            }
        }
        at += best_len;
    }

    let (code, bits) = fixed_literal(256); // end of block
    w.write_code(code, bits);
    w.finish()
}

#[cfg(test)]
#[path = "tests/livery_image.rs"]
mod tests;
