//! Offline first-operator setup: migration-only process, supplied strong credentials, password proof for existing accounts.
use crate::auth;
use crate::store::platform;
use serde_json::json;
use std::env;
use veyndo_core::uid;
pub(crate) async fn bootstrap_operator() {
    assert_eq!(
        env::var("BOOTSTRAP_MODE").as_deref(),
        Ok("migrate"),
        "Operator setup requires BOOTSTRAP_MODE=migrate"
    );
    let v = json!({"email":env::var("PLATFORM_ADMIN_EMAIL").expect("PLATFORM_ADMIN_EMAIL required"),"password":env::var("PLATFORM_ADMIN_PASSWORD").expect("PLATFORM_ADMIN_PASSWORD required"),"name":env::var("PLATFORM_ADMIN_NAME").unwrap_or("Platform operator".into())});
    let email = auth::email(&v).expect("Valid operator email");
    let password = auth::password(&v).expect("Strong operator password required");
    let name = auth::name(&v).expect("Valid operator name");
    let a = crate::bootstrap::bootstrap().await;
    let mut tx = a.db.begin().await.expect("Operator transaction");
    platform::lock_operator_email(&mut tx, &email)
        .await
        .expect("Operator setup lock");
    let existing = platform::account_by_email(&mut tx, &email)
        .await
        .expect("Operator lookup");
    let user = if let Some(row) = existing {
        assert!(
            auth::verify_password(password, row.password_hash)
                .await
                .expect("Password validation"),
            "Existing account password does not match; no operator grant applied"
        );
        row.id
    } else {
        let user = uid();
        let saved = auth::hash_password(password)
            .await
            .expect("Operator password hashing");
        platform::create_account(&mut tx, &user, &email, &name, &saved)
            .await
            .expect("Operator account");
        let slug = format!("operator-{}", &user[..12]);
        auth::provision_shop(
            &mut tx,
            &user,
            &slug,
            "Operator workspace",
            String::new(),
            false,
            false,
        )
        .await
        .expect("Operator workspace");
        user
    };
    platform::grant_operator(&mut tx, &user)
        .await
        .expect("Operator grant");
    platform::audit_operator_bootstrap(&mut tx, &user)
        .await
        .expect("Operator audit");
    tx.commit().await.expect("Operator setup commit");
    println!(
        "Personal operator ready. Sign in at /#platform. No browser/admin token was generated or published."
    );
}
