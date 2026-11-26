//! Idempotent synthetic template catalogue initialization.
use crate::accounts;
use crate::foundation::App;
use crate::store::shops;
use argon2::Argon2;
use argon2::PasswordHasher;
use argon2::password_hash::SaltString;
use serde_json::json;
use uuid::Uuid;
use veyndo_core::Result;

pub(crate) async fn seed(a: &App) -> Result<()> {
    let demo = [
        (
            "lamp",
            "Arc Desk Light",
            "lighting",
            "Warm dimmable light, recycled aluminium.",
            79.9,
            19.,
            40,
        ),
        (
            "chair",
            "Form Chair",
            "furniture",
            "Oak frame, natural linen, compact design.",
            189.,
            19.,
            20,
        ),
        (
            "desk",
            "Studio Desk",
            "furniture",
            "Solid oak workspace with concealed cable tray.",
            449.,
            19.,
            10,
        ),
        (
            "mug",
            "Everyday Cup",
            "objects",
            "Hand-finished stoneware, dishwasher safe.",
            24.9,
            19.,
            80,
        ),
        (
            "notebook",
            "Field Notes",
            "objects",
            "Recycled paper and stitched binding.",
            12.5,
            7.,
            150,
        ),
        (
            "shelf",
            "Line Shelf",
            "furniture",
            "Modular oak storage, wall or desk mounting.",
            119.,
            19.,
            25,
        ),
    ];
    for t in ["atelier", "workshop"] {
        for (id, name, category, description, price, tax, stock) in demo {
            veyndo_catalog::store::demo_template::insert_basic(
                a,
                t,
                (id, name, category, description),
                (price, tax, stock),
            )
            .await?;
        }
        let salt = SaltString::encode_b64(Uuid::new_v4().as_bytes()).unwrap();
        let password_hash = Argon2::default()
            .hash_password(b"demo-business", &salt)
            .unwrap()
            .to_string();
        shops::insert_demo_customer(&a.db, t, &password_hash, true).await?;
        let mut address_tx = a.db.begin().await?;
        accounts::seed_demo_address(&mut address_tx, t).await?;
        address_tx.commit().await?;
        shops::insert_experience(
            &a.db,
            t,
            json!({"mode":"balanced","headline":"Objects for a more considered everyday."}),
            true,
        )
        .await?;
        for v in ["discovery", "comparison"] {
            shops::insert_policy(&a.db, t, v, true).await?;
        }
    }
    Ok(())
}
