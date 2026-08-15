use std::io::Cursor;

use image::{ImageFormat, ImageReader};

use crate::error::{AppError, ErrorKind};
use crate::models::AppResult;

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
