use std::path::Path;

use crate::error::{Error, Result};

/// Filename of the custom profile picture file placed inside a profile directory.
pub const AVATAR_FILE_NAME: &str = "Google Profile Picture.png";

/// Maximum width and height of a normalized profile avatar in pixels.
pub const MAX_DIMENSION: u32 = 256;

/// Reads a PNG or JPEG file from `source`, scales it down if needed so neither
/// dimension exceeds `max_dimension` (preserving aspect ratio and never upscaling),
/// converts it to RGBA8, and returns encoded PNG bytes.
///
/// The source file is opened read-only and never modified.
pub fn normalize_to_png(source: &Path, max_dimension: u32) -> Result<Vec<u8>> {
    let reader = match image::ImageReader::open(source) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(Error::NotFound(source.to_path_buf()));
        }
        Err(e) => return Err(Error::io(source, e)),
    };

    let reader = reader
        .with_guessed_format()
        .map_err(|e| Error::io(source, e))?;

    let img = reader.decode().map_err(|e| Error::Malformed {
        path: source.to_path_buf(),
        message: e.to_string(),
    })?;

    let (width, height) = (img.width(), img.height());
    let resized = if (width > max_dimension || height > max_dimension) && max_dimension > 0 {
        img.resize(
            max_dimension,
            max_dimension,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        img
    };

    let rgba = resized.to_rgba8();
    let dynamic = image::DynamicImage::ImageRgba8(rgba);
    let mut cursor = std::io::Cursor::new(Vec::new());
    dynamic
        .write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| Error::Other(format!("failed to encode avatar to PNG: {e}")))?;

    Ok(cursor.into_inner())
}

/// The `info_cache` entry key-value pairs to set when installing a custom avatar picture.
pub fn local_state_avatar_fields() -> Vec<(&'static str, serde_json::Value)> {
    vec![
        (
            "gaia_picture_file_name",
            serde_json::Value::String(AVATAR_FILE_NAME.to_string()),
        ),
        ("use_gaia_picture", serde_json::Value::Bool(true)),
        ("is_using_default_avatar", serde_json::Value::Bool(false)),
    ]
}

/// The inverse `info_cache` entry key-value pairs for rollback or clearing a custom avatar picture.
pub fn local_state_avatar_clear_fields() -> Vec<(&'static str, serde_json::Value)> {
    vec![
        ("gaia_picture_file_name", serde_json::Value::Null),
        ("use_gaia_picture", serde_json::Value::Bool(false)),
        ("is_using_default_avatar", serde_json::Value::Bool(true)),
    ]
}
