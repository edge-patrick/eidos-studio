use std::io::Cursor;

use image::{ImageFormat, ImageReader};

use crate::error::{AppError, ErrorKind};
use crate::models::AppResult;

pub const HISTORY_THUMBNAIL_EDGE: u32 = 512;

pub fn inspect_raster(bytes: &[u8]) -> AppResult<(&'static str, &'static str, u32, u32)> {
    let format = image::guess_format(bytes).map_err(|_| {
        AppError::new(
            ErrorKind::File,
            "Choose a valid PNG, JPEG, or WebP image.",
            false,
        )
    })?;
    let (mime_type, extension) = match format {
        ImageFormat::Png => ("image/png", "png"),
        ImageFormat::Jpeg => ("image/jpeg", "jpg"),
        ImageFormat::WebP => ("image/webp", "webp"),
        _ => {
            return Err(AppError::new(
                ErrorKind::File,
                "Choose a PNG, JPEG, or WebP image.",
                false,
            ));
        }
    };
    let (width, height) = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(AppError::file)?
        .into_dimensions()
        .map_err(AppError::file)?;
    Ok((mime_type, extension, width, height))
}

pub fn create_history_thumbnail(bytes: &[u8]) -> AppResult<Vec<u8>> {
    let image = image::load_from_memory(bytes).map_err(AppError::file)?;
    let thumbnail = image.thumbnail(HISTORY_THUMBNAIL_EDGE, HISTORY_THUMBNAIL_EDGE);
    let mut output = Cursor::new(Vec::new());
    thumbnail
        .write_to(&mut output, ImageFormat::Png)
        .map_err(AppError::file)?;
    Ok(output.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageBuffer, Rgb};

    #[test]
    fn history_thumbnail_bounds_the_long_edge() {
        let source = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(1024, 512, Rgb([42, 20, 90])));
        let mut encoded = Cursor::new(Vec::new());
        source
            .write_to(&mut encoded, ImageFormat::Png)
            .expect("encode source");

        let thumbnail = create_history_thumbnail(encoded.get_ref()).expect("thumbnail");
        let decoded = image::load_from_memory(&thumbnail).expect("decode thumbnail");

        assert_eq!(decoded.width(), HISTORY_THUMBNAIL_EDGE);
        assert_eq!(decoded.height(), HISTORY_THUMBNAIL_EDGE / 2);
    }
}
