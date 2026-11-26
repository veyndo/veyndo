//! Bounded logo decoding: only raster formats, limited size and memory, re-encoded as PNG so no metadata or active content survives.
use image::ImageFormat;
use std::io::Cursor;
use veyndo_core::{Result, bad};

/// Decodes a PNG, JPEG or WebP logo of at most 2 MiB and 4096 pixels per side and re-encodes it as PNG without metadata.
pub fn normalize(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32)> {
    if bytes.is_empty() || bytes.len() > 2 * 1024 * 1024 {
        return Err(bad("Logo must contain 1 byte..2 MiB"));
    }
    let format = image::guess_format(bytes).map_err(|_| bad("Invalid company image"))?;
    if ![ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::WebP].contains(&format) {
        return Err(bad("Logo requires PNG, JPEG or WebP"));
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let img = reader
        .decode()
        .map_err(|_| bad("Invalid or oversized company image"))?;
    let (w, h) = (img.width(), img.height());
    let mut output = Cursor::new(Vec::new());
    img.write_to(&mut output, ImageFormat::Png)
        .map_err(|_| bad("Cannot normalize logo"))?;
    if output.get_ref().len() > 2 * 1024 * 1024 {
        return Err(bad("Normalized logo exceeds 2 MiB"));
    }
    Ok((output.into_inner(), w, h))
}
#[cfg(test)]
mod tests {
    use super::normalize;
    use image::ImageFormat;
    use std::io::Cursor;
    #[test]
    fn active_content_and_truncated_images_rejected() {
        assert!(normalize(b"<svg onload='alert(1)'/>").is_err());
        assert!(normalize(b"\x89PNG\r\n\x1a\n").is_err());
        assert!(normalize(&vec![0; 2 * 1024 * 1024 + 1]).is_err());
    }
    #[test]
    fn normalized_png_decodes_without_metadata() {
        let img = image::DynamicImage::new_rgba8(4, 3);
        let mut b = Cursor::new(Vec::new());
        img.write_to(&mut b, ImageFormat::Png).unwrap();
        let (out, w, h) = normalize(b.get_ref()).unwrap();
        assert_eq!((w, h), (4, 3));
        assert!(image::load_from_memory(&out).is_ok());
    }
}
