//! The DDS reader and the PNG writer, checked against each other.
//!
//! Every test here goes the whole way round — a `.dds` built by hand, through
//! [`dds_thumbnail_png`], and back out through the inflater and unfilterer below. Checking the
//! encoder's output only for a plausible header would pass just as happily on a stream no
//! browser could read, which is exactly the failure this file exists to catch.

use super::*;

// ── reading the PNG back ─────────────────────────────────────────────────────

/// Bits come out least-significant first, and a Huffman code most-significant first — the mirror
/// of [`BitWriter`].
struct BitReader<'a> {
    data: &'a [u8],
    at: usize,
}

impl BitReader<'_> {
    fn bit(&mut self) -> u32 {
        let byte = self.data[self.at / 8];
        let bit = (byte >> (self.at % 8)) & 1;
        self.at += 1;
        bit as u32
    }

    fn bits(&mut self, count: u32) -> u32 {
        (0..count).map(|i| self.bit() << i).sum()
    }

    fn code(&mut self, count: u32) -> u32 {
        (0..count).fold(0, |acc, _| (acc << 1) | self.bit())
    }
}

/// Inflates one fixed-Huffman deflate block — all this encoder ever writes.
fn inflate_fixed(stream: &[u8]) -> Vec<u8> {
    let mut r = BitReader { data: stream, at: 0 };
    assert_eq!(r.bits(1), 1, "the encoder writes a single final block");
    assert_eq!(r.bits(2), 1, "the encoder writes fixed Huffman blocks");
    let mut out: Vec<u8> = Vec::new();
    loop {
        let mut value = r.code(7);
        let symbol = if value <= 0x17 {
            256 + value
        } else {
            value = (value << 1) | r.bit();
            if (0x30..=0xBF).contains(&value) {
                value - 0x30
            } else if (0xC0..=0xC7).contains(&value) {
                280 + value - 0xC0
            } else {
                144 + ((value << 1) | r.bit()) - 0x190
            }
        };
        match symbol {
            0..=255 => out.push(symbol as u8),
            256 => return out,
            _ => {
                let li = (symbol - 257) as usize;
                let length = LENGTH_BASE[li] as usize + r.bits(LENGTH_EXTRA[li]) as usize;
                let di = r.code(5) as usize;
                let distance = DISTANCE_BASE[di] as usize + r.bits(DISTANCE_EXTRA[di]) as usize;
                for _ in 0..length {
                    out.push(out[out.len() - distance]);
                }
            }
        }
    }
}

/// Width, height and RGBA bytes of a PNG this module wrote.
fn read_png(png: &[u8]) -> (u32, u32, Vec<u8>) {
    assert_eq!(&png[0..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    let mut at = 8;
    let (mut w, mut h) = (0u32, 0u32);
    let mut compressed: Vec<u8> = Vec::new();
    while at + 8 <= png.len() {
        let len = u32::from_be_bytes([png[at], png[at + 1], png[at + 2], png[at + 3]]) as usize;
        let kind = &png[at + 4..at + 8];
        let body = &png[at + 8..at + 8 + len];
        let crc = u32::from_be_bytes([
            png[at + 8 + len],
            png[at + 9 + len],
            png[at + 10 + len],
            png[at + 11 + len],
        ]);
        assert_eq!(crc, crc32(&png[at + 4..at + 8 + len]), "chunk CRC");
        match kind {
            b"IHDR" => {
                w = u32::from_be_bytes([body[0], body[1], body[2], body[3]]);
                h = u32::from_be_bytes([body[4], body[5], body[6], body[7]]);
                assert_eq!(&body[8..], &[8, 6, 0, 0, 0], "8-bit RGBA, no interlace");
            }
            b"IDAT" => compressed.extend_from_slice(body),
            _ => {}
        }
        at += 12 + len;
    }
    assert_eq!(&compressed[0..2], &[0x78, 0x01], "zlib header");
    let raw = inflate_fixed(&compressed[2..compressed.len() - 4]);
    let stored = u32::from_be_bytes([
        compressed[compressed.len() - 4],
        compressed[compressed.len() - 3],
        compressed[compressed.len() - 2],
        compressed[compressed.len() - 1],
    ]);
    assert_eq!(stored, adler32(&raw), "zlib checksum");

    let stride = w as usize * 4;
    let mut pixels = vec![0u8; h as usize * stride];
    for y in 0..h as usize {
        let filter = raw[y * (stride + 1)];
        let line = &raw[y * (stride + 1) + 1..(y + 1) * (stride + 1)];
        for i in 0..stride {
            let left = if i < 4 { 0 } else { pixels[y * stride + i - 4] };
            let up = if y == 0 { 0 } else { pixels[(y - 1) * stride + i] };
            let up_left = if y == 0 || i < 4 {
                0
            } else {
                pixels[(y - 1) * stride + i - 4]
            };
            pixels[y * stride + i] = line[i].wrapping_add(match filter {
                0 => 0,
                1 => left,
                2 => up,
                3 => ((left as u16 + up as u16) / 2) as u8,
                4 => paeth(left, up, up_left),
                other => panic!("unknown PNG filter {other}"),
            });
        }
    }
    (w, h, pixels)
}

// ── building a DDS by hand ───────────────────────────────────────────────────

/// A DDS header with the given dimensions. `fourcc` empty means uncompressed, in which case the
/// masks and bit count describe the layout instead.
fn header(w: u32, h: u32, fourcc: &[u8], bits: u32, masks: [u32; 4]) -> Vec<u8> {
    let mut dds = vec![0u8; 128];
    dds[0..4].copy_from_slice(b"DDS ");
    dds[4..8].copy_from_slice(&124u32.to_le_bytes());
    dds[12..16].copy_from_slice(&h.to_le_bytes());
    dds[16..20].copy_from_slice(&w.to_le_bytes());
    dds[76..80].copy_from_slice(&32u32.to_le_bytes());
    if fourcc.is_empty() {
        dds[80..84].copy_from_slice(&0x41u32.to_le_bytes()); // DDPF_RGB | DDPF_ALPHAPIXELS
        dds[88..92].copy_from_slice(&bits.to_le_bytes());
        for (i, mask) in masks.iter().enumerate() {
            dds[92 + i * 4..96 + i * 4].copy_from_slice(&mask.to_le_bytes());
        }
    } else {
        dds[80..84].copy_from_slice(&0x4u32.to_le_bytes()); // DDPF_FOURCC
        dds[84..88].copy_from_slice(fourcc);
    }
    dds
}

fn to565(r: u8, g: u8, b: u8) -> u16 {
    ((r as u16 >> 3) << 11) | ((g as u16 >> 2) << 5) | (b as u16 >> 3)
}

/// One 4×4 BC1 block: two endpoint colours and the sixteen two-bit indices, low pixel first.
fn bc1_block(c0: u16, c1: u16, indices: [u8; 16]) -> Vec<u8> {
    let mut block = Vec::new();
    block.extend_from_slice(&c0.to_le_bytes());
    block.extend_from_slice(&c1.to_le_bytes());
    let packed: u32 = indices
        .iter()
        .enumerate()
        .map(|(i, &v)| (v as u32) << (2 * i))
        .sum();
    block.extend_from_slice(&packed.to_le_bytes());
    block
}

/// One 4×4 BC3 alpha block: two endpoints and sixteen three-bit indices.
fn bc3_alpha(a0: u8, a1: u8, indices: [u8; 16]) -> Vec<u8> {
    let packed: u64 = indices
        .iter()
        .enumerate()
        .map(|(i, &v)| (v as u64) << (3 * i))
        .sum();
    let mut block = vec![a0, a1];
    block.extend_from_slice(&packed.to_le_bytes()[0..6]);
    block
}

fn pixel(rgba: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * w + x) * 4) as usize;
    [rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3]]
}

// ── the tests ────────────────────────────────────────────────────────────────

#[test]
fn test_a_bc1_block_keeps_its_two_endpoint_colours() {
    let red = to565(255, 0, 0);
    let blue = to565(0, 0, 255);
    // Endpoint 0 on the top row, endpoint 1 everywhere else. c0 > c1 so there is no hole.
    let mut indices = [1u8; 16];
    indices[0..4].copy_from_slice(&[0, 0, 0, 0]);
    let mut dds = header(4, 4, b"DXT1", 0, [0; 4]);
    dds.extend(bc1_block(red.max(blue), red.min(blue), indices));
    assert!(red > blue, "this test needs red as the larger endpoint");

    let png = dds_thumbnail_png(&dds, 4).unwrap();
    let (w, h, pixels) = read_png(&png);
    assert_eq!((w, h), (4, 4));
    assert_eq!(pixel(&pixels, w, 0, 0), [255, 0, 0, 255]);
    assert_eq!(pixel(&pixels, w, 3, 3), [0, 0, 255, 255]);
}

#[test]
fn test_a_bc1_hole_is_transparent_rather_than_a_fourth_colour() {
    let red = to565(255, 0, 0);
    let blue = to565(0, 0, 255);
    let (low, high) = (red.min(blue), red.max(blue));
    // c0 <= c1 is what puts a BC1 block in punch-through mode: index 3 is then a hole.
    let mut indices = [0u8; 16];
    indices[15] = 3;
    let mut dds = header(4, 4, b"DXT1", 0, [0; 4]);
    dds.extend(bc1_block(low, high, indices));

    let png = dds_thumbnail_png(&dds, 4).unwrap();
    let (w, _, pixels) = read_png(&png);
    assert_eq!(pixel(&pixels, w, 3, 3)[3], 0, "the hole is transparent");
    assert_eq!(pixel(&pixels, w, 0, 0)[3], 255, "the rest is not");
}

#[test]
fn test_bc3_reads_its_alpha_from_its_own_block() {
    let white = to565(255, 255, 255);
    let mut alpha_indices = [0u8; 16];
    alpha_indices[15] = 1; // the second endpoint
    let mut dds = header(4, 4, b"DXT5", 0, [0; 4]);
    dds.extend(bc3_alpha(200, 40, alpha_indices));
    dds.extend(bc1_block(white, white, [0; 16]));

    let png = dds_thumbnail_png(&dds, 4).unwrap();
    let (w, _, pixels) = read_png(&png);
    assert_eq!(pixel(&pixels, w, 0, 0)[3], 200);
    assert_eq!(pixel(&pixels, w, 3, 3)[3], 40);
}

#[test]
fn test_bc2_reads_its_alpha_as_four_bit_nibbles() {
    let white = to565(255, 255, 255);
    let mut dds = header(4, 4, b"DXT3", 0, [0; 4]);
    // The low nibble of a byte is the earlier pixel: pixel 0 opaque, pixel 1 transparent, the
    // rest half way.
    let mut alpha = vec![0x0Fu8];
    alpha.extend(std::iter::repeat_n(0x88u8, 7));
    dds.extend(alpha);
    dds.extend(bc1_block(white, white, [0; 16]));

    let png = dds_thumbnail_png(&dds, 4).unwrap();
    let (w, _, pixels) = read_png(&png);
    assert_eq!(pixel(&pixels, w, 0, 0)[3], 255, "0xF widens to 255, not 240");
    assert_eq!(pixel(&pixels, w, 1, 0)[3], 0);
    assert_eq!(pixel(&pixels, w, 2, 0)[3], 136);
}

#[test]
fn test_an_uncompressed_dds_is_read_by_its_masks() {
    // BGRA, the order a DDS usually stores 32-bit pixels in.
    let mut dds = header(2, 1, b"", 32, [0x00FF_0000, 0x0000_FF00, 0x0000_00FF, 0xFF00_0000]);
    dds.extend([0x00, 0x00, 0xFF, 0xFF]); // red, opaque
    dds.extend([0xFF, 0x00, 0x00, 0x80]); // blue, half transparent

    let png = dds_thumbnail_png(&dds, 8).unwrap();
    let (w, h, pixels) = read_png(&png);
    assert_eq!((w, h), (2, 1), "a small source is not scaled up");
    assert_eq!(pixel(&pixels, w, 0, 0), [255, 0, 0, 255]);
    assert_eq!(pixel(&pixels, w, 1, 0), [0, 0, 255, 128]);
}

#[test]
fn test_scaling_weights_colour_by_alpha_so_the_background_cannot_darken_the_car() {
    // Four pixels in a row: one opaque white, three transparent black — the situation at every
    // edge of a car on a transparent background. An unweighted average would give 64, a grey
    // fringe round the bodywork.
    let white = to565(255, 255, 255);
    let mut dds = header(4, 4, b"DXT5", 0, [0; 4]);
    let mut alpha_indices = [1u8; 16]; // endpoint 1 = transparent
    alpha_indices[0] = 0;
    dds.extend(bc3_alpha(255, 0, alpha_indices));
    dds.extend(bc1_block(white, to565(0, 0, 0), [0; 16]));

    let png = dds_thumbnail_png(&dds, 1).unwrap();
    let (w, h, pixels) = read_png(&png);
    assert_eq!((w, h), (1, 1));
    let [r, g, b, a] = pixel(&pixels, w, 0, 0);
    assert_eq!([r, g, b], [255, 255, 255], "the one visible pixel's colour");
    assert_eq!(a, 15, "one opaque pixel in sixteen");
}

#[test]
fn test_the_thumbnail_keeps_the_aspect_ratio_and_the_requested_width() {
    let white = to565(255, 255, 255);
    let mut dds = header(64, 16, b"DXT1", 0, [0; 4]);
    for _ in 0..(16 * 4) {
        dds.extend(bc1_block(white, white, [0; 16]));
    }

    let png = dds_thumbnail_png(&dds, 16).unwrap();
    let (w, h, _) = read_png(&png);
    assert_eq!((w, h), (16, 4));
}

#[test]
fn test_an_unsupported_format_is_reported_rather_than_guessed_at() {
    let dds = header(4, 4, b"DX10", 0, [0; 4]);
    let err = dds_thumbnail_png(&dds, 4).unwrap_err();
    assert!(err.contains("DX10"), "{err}");

    let err = dds_thumbnail_png(b"not a texture at all", 4).unwrap_err();
    assert!(err.contains("not a DDS"), "{err}");
}

#[test]
fn test_a_truncated_dds_is_refused_rather_than_read_past_its_end() {
    let mut dds = header(64, 64, b"DXT5", 0, [0; 4]);
    dds.extend([0u8; 16]); // one block where 256 are declared
    let err = dds_thumbnail_png(&dds, 32).unwrap_err();
    assert!(err.contains("shorter"), "{err}");
}

#[test]
fn test_the_compressor_round_trips_every_byte_value_and_long_repeats() {
    // A run far longer than one match, a stretch of every byte value (which reaches both halves
    // of the fixed literal code), and a repeat at a distance needing extra bits.
    let mut raw: Vec<u8> = vec![0u8; 1000];
    raw.extend((0..=255u8).cycle().take(4096));
    let tail = raw[100..900].to_vec();
    raw.extend(tail);
    let encoded = zlib(&raw);
    assert_eq!(inflate_fixed(&encoded[2..encoded.len() - 4]), raw);
    assert!(
        encoded.len() < raw.len(),
        "{} bytes from {}",
        encoded.len(),
        raw.len()
    );
}

#[test]
fn test_every_png_filter_survives_the_round_trip() {
    // Rows chosen so the filter heuristic picks a different one for each: flat, a horizontal
    // ramp, a copy of the row above, and noise.
    let (w, h) = (32usize, 4usize);
    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let at = (y * w + x) * 4;
            let v = match y {
                0 => 7,
                1 => (x * 8) as u8,
                2 => (x * 8) as u8,
                _ => ((x * 37 + 11) % 251) as u8,
            };
            rgba[at..at + 4].copy_from_slice(&[v, v.wrapping_add(1), v.wrapping_add(2), 255]);
        }
    }
    let png = encode_png(w as u32, h as u32, &rgba);
    let (rw, rh, back) = read_png(&png);
    assert_eq!((rw as usize, rh as usize), (w, h));
    assert_eq!(back, rgba);
}
