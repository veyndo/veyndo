//! Binary assets are immutable, staged independently through metadata/digest units; paid entitlements never clone.
pub(super) use veyndo_studio::store::staging_assets::validate_product_media;
pub(crate) use veyndo_studio::store::staging_assets::{publish_asset, snapshot_assets};
