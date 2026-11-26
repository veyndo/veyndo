//! File admission by content: a declared type is only accepted when the bytes carry its signature.
use crate::domain::model_format;
use veyndo_core::{Result, bad};

/// The content must be what the declared type says; types outside the closed list are refused.
pub fn validate_bytes(mime: &str, b: &[u8]) -> Result<()> {
    if mime == "model/gltf-binary" {
        return model_format::validate(b);
    }
    if b.is_empty() || b.len() > 8 * 1024 * 1024 {
        return Err(bad("Asset must contain 1 byte..8 MiB"));
    }
    let ok = match mime {
        "application/pdf" => b.starts_with(b"%PDF-"),
        "image/png" => b.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => b.starts_with(&[255, 216, 255]),
        "image/webp" => b.starts_with(b"RIFF") && b.get(8..12) == Some(b"WEBP"),
        "video/mp4" => b.get(4..8) == Some(b"ftyp"),
        "application/zip" => b.starts_with(b"PK\x03\x04"),
        "text/plain" => std::str::from_utf8(b).is_ok(),
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err(bad("Unsupported file type or mismatched content"))
    }
}

/// Whether a file of `owner` may be delivered in the context of `selected`: the product itself,
/// or one of its variants when the file is a 3D model the variant shows with its own material.
pub fn deliverable_for(
    owner: &str,
    selected: &str,
    selected_parent: Option<&str>,
    mime: &str,
) -> bool {
    selected == owner || (mime == "model/gltf-binary" && selected_parent == Some(owner))
}

#[cfg(test)]
mod tests {
    use super::validate_bytes;

    #[test]
    fn unsafe_files_rejected() {
        assert!(validate_bytes("text/html", b"<script>bad</script>").is_err());
        assert!(validate_bytes("application/pdf", b"bad").is_err());
        assert!(validate_bytes("text/plain", b"Manual").is_ok());
        assert!(validate_bytes("image/png", b"\x89PNG\r\n\x1a\nrest").is_ok());
        assert!(validate_bytes("image/png", b"").is_err());
    }

    #[test]
    fn a_variant_may_load_only_the_model_of_its_parent() {
        use super::deliverable_for;
        assert!(deliverable_for("chair", "chair", None, "application/pdf"));
        assert!(deliverable_for(
            "chair",
            "chair-oak",
            Some("chair"),
            "model/gltf-binary"
        ));
        assert!(!deliverable_for(
            "chair",
            "chair-oak",
            Some("chair"),
            "application/pdf"
        ));
        assert!(!deliverable_for(
            "chair",
            "table",
            None,
            "model/gltf-binary"
        ));
        assert!(!deliverable_for(
            "chair",
            "lamp-red",
            Some("lamp"),
            "model/gltf-binary"
        ));
    }
}
