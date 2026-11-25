//! Bounded self-contained GLB 2 admission; remote resources and runtime decoder downloads are forbidden.
use serde::Deserialize;
use serde_json::Value;
use veyndo_core::document::{lenient, lenient_list, view};
use veyndo_core::{Result, bad};
pub const MODEL_LIMIT: usize = 20 * 1024 * 1024;
const JSON_LIMIT: usize = 1024 * 1024;
fn word(b: &[u8], offset: usize) -> Result<usize> {
    Ok(u32::from_le_bytes(
        b.get(offset..offset + 4)
            .ok_or(bad("Invalid GLB structure"))?
            .try_into()
            .unwrap(),
    ) as usize)
}
pub fn document(b: &[u8]) -> Result<Value> {
    if b.get(..4) != Some(b"glTF") || word(b, 4)? != 2 || b.get(16..20) != Some(b"JSON") {
        return Err(bad("A GLB 2 model is required"));
    }
    let len = word(b, 12)?;
    if len == 0 || len > JSON_LIMIT || len % 4 != 0 {
        return Err(bad("GLB metadata exceeds limits"));
    }
    serde_json::from_slice(b.get(20..20 + len).ok_or(bad("Invalid GLB metadata"))?)
        .map_err(|_| bad("Invalid GLB metadata"))
}
fn safe_tree(v: &Value, depth: usize, nodes: &mut usize) -> bool {
    *nodes += 1;
    if depth > 32 || *nodes > 50000 {
        return false;
    }
    match v {
        Value::Object(o) => o.iter().all(|(k, v)| {
            ![
                "uri",
                "KHR_draco_mesh_compression",
                "EXT_meshopt_compression",
                "KHR_texture_basisu",
            ]
            .contains(&k.as_str())
                && safe_tree(v, depth + 1, nodes)
        }),
        Value::Array(a) => a.iter().all(|v| safe_tree(v, depth + 1, nodes)),
        _ => true,
    }
}
/// A material variant of a GLB document.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Variant {
    #[serde(deserialize_with = "lenient")]
    name: Option<String>,
}

/// The parts of a GLB document whose sizes are bounded.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Limits {
    #[serde(deserialize_with = "lenient_list")]
    buffers: Vec<Buffer>,
    #[serde(deserialize_with = "lenient_list")]
    accessors: Vec<Accessor>,
}

/// A GLB buffer and its declared length.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Buffer {
    #[serde(deserialize_with = "lenient")]
    byte_length: Option<u64>,
}

/// A GLB accessor and its element count.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Accessor {
    #[serde(deserialize_with = "lenient")]
    count: Option<u64>,
}

pub fn variants(doc: &Value) -> Result<Vec<String>> {
    let mut names = vec![];
    if let Some(v) = doc.pointer("/extensions/KHR_materials_variants/variants") {
        let list = v
            .as_array()
            .filter(|a| a.len() <= 100)
            .ok_or(bad("Invalid GLB material variants"))?;
        for item in list {
            let variant: Variant = view(item);
            let name = variant
                .name
                .filter(|n| !n.is_empty() && n.len() <= 100 && !n.chars().any(char::is_control))
                .ok_or(bad("Invalid GLB material variant name"))?;
            if names.contains(&name) {
                return Err(bad("Duplicate GLB material variant"));
            }
            names.push(name);
        }
    }
    Ok(names)
}
pub fn validate(b: &[u8]) -> Result<()> {
    if b.len() > MODEL_LIMIT || b.len() < 20 || word(b, 8)? != b.len() {
        return Err(bad("GLB must contain a complete model up to 20 MiB"));
    }
    let doc = document(b)?;
    if doc.pointer("/asset/version").and_then(Value::as_str) != Some("2.0")
        || !safe_tree(&doc, 0, &mut 0)
    {
        return Err(bad(
            "GLB must embed all resources without external decoders",
        ));
    }
    variants(&doc)?;
    let offset = 20 + word(b, 12)?;
    let bin = if offset == b.len() {
        0
    } else {
        let len = word(b, offset)?;
        if b.get(offset + 4..offset + 8) != Some(b"BIN\0")
            || len % 4 != 0
            || offset + 8 + len != b.len()
        {
            return Err(bad("Invalid GLB binary chunk"));
        }
        len
    };
    let limits: Limits = view(&doc);
    if limits.buffers.len() > 1
        || limits.buffers.iter().any(|v| {
            v.byte_length
                .is_none_or(|n| n as usize > bin || bin - n as usize > 3)
        })
    {
        return Err(bad("Invalid GLB buffer size"));
    }
    if limits
        .accessors
        .iter()
        .any(|v| v.count.is_none_or(|n| n > 2_000_000))
    {
        return Err(bad("GLB geometry exceeds limits"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{MODEL_LIMIT, document, validate, variants};
    use crate::domain::media::validate_shape;
    use serde_json::{Value, json};
    fn file(value: Value) -> Vec<u8> {
        let mut text = serde_json::to_vec(&value).unwrap();
        while !text.len().is_multiple_of(4) {
            text.push(b' ');
        }
        let mut bytes = b"glTF".to_vec();
        bytes.extend(2_u32.to_le_bytes());
        bytes.extend(((20 + text.len()) as u32).to_le_bytes());
        bytes.extend((text.len() as u32).to_le_bytes());
        bytes.extend(b"JSON");
        bytes.extend(text);
        bytes
    }
    #[test]
    fn embedded_models_and_material_variants() {
        let bytes = include_bytes!("../../../../fixtures/media/3d-stool.glb");
        assert!(validate(bytes).is_ok());
        assert_eq!(
            variants(&document(bytes).unwrap()).unwrap(),
            vec!["Oak", "Ocean"]
        );
        let mut broken = bytes.to_vec();
        broken[8] = 0;
        assert!(validate(&broken).is_err());
        assert!(validate(b"glTF").is_err());
    }
    #[test]
    fn external_resources_decoders_and_unbounded_geometry_are_rejected() {
        for value in [
            json!({"asset":{"version":"2.0"},"buffers":[{"uri":"https://example.test/model.bin","byteLength":4}]}),
            json!({"asset":{"version":"2.0"},"images":[{"uri":"data:image/svg+xml,bad"}]}),
            json!({"asset":{"version":"2.0"},"extensions":{"KHR_draco_mesh_compression":{}}}),
            json!({"asset":{"version":"2.0"},"accessors":[{"count":2000001}]}),
            json!({"asset":{"version":"2.0"},"extensions":{"KHR_materials_variants":{"variants":[{"name":"Oak"},{"name":"Oak"}]}}}),
        ] {
            assert!(validate(&file(value)).is_err());
        }
        let mut huge = include_bytes!("../../../../fixtures/media/3d-stool.glb").to_vec();
        huge.resize(MODEL_LIMIT + 1, 0);
        assert!(validate(&huge).is_err());
        assert!(
            validate_shape(&[json!({"type":"model","url":"https://example.test/a.glb"})]).is_err()
        );
        assert!(validate_shape(&[json!({"type":"video","url":"/media/a"})]).is_err());
    }
}
