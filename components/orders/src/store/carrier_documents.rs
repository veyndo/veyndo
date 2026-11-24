//! Immutable private carrier documents belong to the existing order receipt store, never product assets.
use crate::store::order_mutation::{self, Audit, Tx};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::document::{lenient, view};
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, conflict, hash, uid};

/// The digest a carrier app must echo: it changes whenever the package, its method or the destination changes.
/// The order fields a carrier document allocation is bound to.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct AllocatedOrder {
    id: Value,
    shipping_address: Value,
    #[serde(deserialize_with = "lenient")]
    deliveries: Option<Vec<Value>>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct AllocatedDelivery {
    positions: Value,
    #[serde(deserialize_with = "lenient")]
    shipping_method: MethodId,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct MethodId {
    id: Value,
}

pub fn allocation_digest(order: &Value, index: usize) -> String {
    let order: AllocatedOrder = view(order);
    let delivery: AllocatedDelivery = order
        .deliveries
        .as_ref()
        .and_then(|ds| ds.get(index))
        .map(view)
        .unwrap_or_default();
    hash(&json!({"id":order.id,"deliveryIndex":index,"positions":delivery.positions,"method":delivery.shipping_method.id,"address":order.shipping_address}).to_string())
}

/// A carrier document an approved app hands in, validated before any lock is taken.
pub struct Document<'a> {
    id: &'a str,
    pub key: &'a str,
    kind: &'a str,
    index: usize,
    tracking: &'a str,
    url: &'a str,
    fingerprint: String,
    metadata: &'a Value,
}
impl<'a> Document<'a> {
    pub fn admit(
        app: &str,
        metadata: &'a Value,
        content: &[u8],
        safe_url: &dyn Fn(&str) -> bool,
    ) -> Result<Self> {
        let id = metadata["orderId"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 100)
            .ok_or(bad("Order ID required"))?;
        let key = metadata["requestKey"]
            .as_str()
            .filter(|s| (8..=128).contains(&s.len()))
            .ok_or(bad("Document request key required"))?;
        let kind = metadata["kind"]
            .as_str()
            .filter(|s| ["shipping_label", "return_label"].contains(s))
            .ok_or(bad("Invalid carrier document kind"))?;
        let index = metadata["deliveryIndex"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or(bad("Shipment index required"))?;
        if !content.starts_with(b"%PDF-") || content.len() > 2 * 1024 * 1024 {
            return Err(bad("Carrier document must be a PDF within 2 MiB"));
        }
        let tracking = metadata["trackingCode"].as_str().unwrap_or("");
        let url = metadata["trackingUrl"].as_str().unwrap_or("");
        if tracking.len() > 100
            || tracking.chars().any(char::is_control)
            || url.len() > 2000
            || !url.is_empty() && (!url.starts_with("https://") || !safe_url(url))
        {
            return Err(bad("Invalid carrier tracking information"));
        }
        let content_digest = {
            use sha2::Digest;
            format!("{:x}", sha2::Sha256::digest(content))
        };
        let fingerprint =
            hash(&json!({"app":app,"metadata":metadata,"content":content_digest}).to_string());
        Ok(Self {
            id,
            key,
            kind,
            index,
            tracking,
            url,
            fingerprint,
            metadata,
        })
    }
}

/// Stores the document once per request key inside the app's package write transaction and marks the package as labelled.
pub async fn record(
    tx: &mut Tx<'_>,
    t: &str,
    h: &RequestContext,
    app: &str,
    version: &str,
    document: Document<'_>,
    content: Vec<u8>,
) -> Result<Value> {
    let Document {
        id,
        key,
        kind,
        index,
        tracking,
        url,
        fingerprint,
        metadata,
    } = document;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("receipt:{t}:{key}"))
        .execute(&mut **tx)
        .await?;
    if let Some(r)=sqlx::query("SELECT id,number,fingerprint,source_app FROM order_receipts WHERE tenant=$1 AND request_key=$2").bind(t).bind(key).fetch_optional(&mut **tx).await? {
        if r.get::<String,_>("fingerprint")!=fingerprint || r.get::<Option<String>,_>("source_app").as_deref()!=Some(app) {return Err(conflict("Document request key reused"));}
        return Ok(json!({"id":r.get::<String,_>("id"),"number":r.get::<String,_>("number")}));
    }
    let mut order: Value =
        sqlx::query_scalar("SELECT data FROM orders WHERE tenant=$1 AND id=$2 FOR UPDATE")
            .bind(t)
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or(Error(StatusCode::NOT_FOUND, "Order not found".into()))?;
    if metadata["allocationDigest"] != allocation_digest(&order, index) {
        return Err(conflict(
            "Package allocation changed; reconcile the provider result before booking again",
        ));
    }
    let d = order["deliveries"]
        .as_array_mut()
        .and_then(|v| v.get_mut(index))
        .ok_or(bad("Shipment unavailable"))?;
    if kind == "shipping_label" && (d["state"] != "open" || !d["label"].is_null()) {
        return Err(conflict(
            "Package already labelled or dispatched; reconcile the existing booking",
        ));
    }
    if kind == "shipping_label"
        && (d["booking"]["sourceApp"] != app || d["booking"]["requestKey"] != key)
    {
        return Err(conflict(
            "Document does not acknowledge this package's original booking",
        ));
    }
    let document = uid();
    let number = format!("DOC-{document}");
    let snapshot = json!({"id":document,"number":number,"kind":kind,"sourceApp":app,"packageVersion":version,"deliveryIndex":index,"allocationDigest":metadata["allocationDigest"],"trackingCode":tracking,"trackingUrl":url});
    // Outbound labels contain recipient data and are merchant-only. Return labels are available solely to the owning account.
    sqlx::query("INSERT INTO order_receipts(id,tenant,order_id,kind,number,locale,snapshot,request_key,fingerprint,content,source_app,customer_visible) VALUES($1,$2,$3,$4,$5,'en',$6,$7,$8,$9,$10,$11)").bind(&document).bind(t).bind(id).bind(kind).bind(&number).bind(&snapshot).bind(key).bind(fingerprint).bind(content).bind(app).bind(kind=="return_label").execute(&mut **tx).await?;
    if kind == "shipping_label" {
        d.as_object_mut().unwrap().remove("booking");
        d["label"] = json!({"id":document,"sourceApp":app});
        d["trackingCode"] = json!(tracking);
        d["trackingUrl"] = if url.is_empty() {
            Value::Null
        } else {
            json!(url)
        };
    }
    order["revision"] = json!(order["revision"].as_u64().unwrap_or(1) + 1);
    order_mutation::persist(tx,t,h,&mut order,Audit {
        kind:"receipt", data:json!({"id":document,"number":number,"kind":kind,"state":"issued","sourceApp":app}),
        event_kind:"document.created",event:json!({"orderId":id,"documentId":document,"kind":kind,"sourceApp":app}),
    }).await?;
    Ok(json!({"id":document,"number":number}))
}
