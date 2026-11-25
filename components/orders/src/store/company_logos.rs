//! Tenant-owned company logos: deduplicated immutable PNGs, delivered publicly only while a channel links them.
use crate::store::company;
use veyndo_core::{Error, Result, State, StatusCode, bad, uid};

/// Stores a normalized logo once per content digest and returns its ID; a shop keeps at most 50.
pub async fn save(
    a: &State,
    t: &str,
    content: Vec<u8>,
    digest: &str,
    width: u32,
    height: u32,
) -> Result<String> {
    let mut tx = a.db.begin().await?;
    company::lock(&mut tx, t).await?;
    let count: i64 = sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM company_logos WHERE tenant=$1"#,
        t
    )
    .fetch_one(&mut *tx)
    .await?;
    let exists: Option<String> = sqlx::query_scalar!(
        "SELECT id FROM company_logos WHERE tenant=$1 AND digest=$2",
        t,
        digest
    )
    .fetch_optional(&mut *tx)
    .await?;
    let id = if let Some(id) = exists {
        id
    } else {
        if count >= 50 {
            return Err(bad("Maximum 50 company logos per shop"));
        }
        let id = uid();
        sqlx::query!("INSERT INTO company_logos(tenant,id,content,digest,mime,width,height) VALUES($1,$2,$3,$4,'image/png',$5,$6)", t, id, content, digest, width as i32, height as i32).execute(&mut *tx).await?;
        id
    };
    tx.commit().await?;
    Ok(id)
}

pub async fn content(a: &State, t: &str, id: &str) -> Result<Vec<u8>> {
    sqlx::query_scalar!(
        "SELECT content FROM company_logos WHERE tenant=$1 AND id=$2",
        t,
        id
    )
    .fetch_optional(&a.db)
    .await?
    .ok_or(Error(
        StatusCode::NOT_FOUND,
        "Company logo not found".into(),
    ))
}

/// A logo is public only on an active channel whose effective company data links it.
pub async fn admit_public(a: &State, t: &str, channel: &str, id: &str) -> Result<()> {
    if channel != "default" {
        let active:bool=sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM sales_channels WHERE tenant=$1 AND id=$2 AND data->>'active'='true') AS "exists!""#, t, channel).fetch_one(&a.db).await?;
        if !active {
            return Err(Error(
                StatusCode::NOT_FOUND,
                "Sales channel not found".into(),
            ));
        }
    }
    let mut tx = a.db.begin().await?;
    let effective = company::seller(&mut tx, t, channel).await?;
    tx.commit().await?;
    if effective["logoId"] != id {
        return Err(Error(
            StatusCode::NOT_FOUND,
            "Company logo is not linked to this channel".into(),
        ));
    }
    Ok(())
}
