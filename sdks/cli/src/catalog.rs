//! Product and order reads, and CSV import through the native catalogue intake into a private sandbox (never the live shop).
use crate::failure::{Exit, Failure, Outcome};
use crate::output::{self, Column, column};
use crate::{Run, sandbox};
use clap::{Subcommand, ValueEnum};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use veyndo_sdk::{Client, RequestOptions};

/// The intake accepts at most 200 kB of source text per reviewed batch.
const SOURCE_LIMIT: u64 = 200_000;

/// `veyndo products …`
#[derive(Subcommand, Debug)]
pub enum ProductsCommand {
    /// List products of the selected shop (or a sandbox)
    List {
        /// Text search in names and product numbers
        #[arg(long)]
        search: Option<String>,
        /// Read a sandbox instead of the live shop
        #[arg(long)]
        sandbox: Option<String>,
        /// Maximum number of products (fetched page by page)
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// Show one product
    Get {
        /// Product ID
        id: String,
        /// Read a sandbox instead of the live shop
        #[arg(long)]
        sandbox: Option<String>,
    },
    /// Import products from a CSV file into a private sandbox for review
    #[command(after_help = "\
CSV columns: name,price required; description,productNumber,imageUrl optional (UTF-8, at most
50 rows and 200 kB). `--format shopify` reads a Shopify product export instead. Prices are in the
shop currency. The rows become new, active products in a private sandbox; nothing changes in the
live shop until you review `veyndo sandbox diff` and run `veyndo sandbox release`.
Re-running the same import into the same sandbox replays instead of duplicating products.")]
    Import {
        /// CSV file
        file: PathBuf,
        /// Import into this sandbox (default: create a new one)
        #[arg(long)]
        sandbox: Option<String>,
        /// Tax rate in percent (default: the shop's only configured rate)
        #[arg(long)]
        tax_rate: Option<f64>,
        /// Source layout
        #[arg(long, value_enum, default_value_t = CsvFormat::Veyndo)]
        format: CsvFormat,
    },
}

/// Supported CSV layouts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum CsvFormat {
    /// name,price[,description,productNumber,imageUrl]
    Veyndo,
    /// A Shopify product export
    Shopify,
}

/// `veyndo orders …`
#[derive(Subcommand, Debug)]
pub enum OrdersCommand {
    /// List orders of the selected shop
    List {
        /// Search by order number or customer
        #[arg(long)]
        search: Option<String>,
        /// Only orders in this state (for example open, completed, cancelled)
        #[arg(long)]
        state: Option<String>,
        /// Maximum number of orders (fetched page by page)
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// Show one order
    Get {
        /// Order ID
        id: String,
    },
}

const PRODUCT_COLUMNS: &[Column] = &[
    column("ID", &["/id"]),
    column("NUMBER", &["/productNumber"]),
    column("NAME", &["/name", "/translated/name"]),
    column("PRICE", &["/price", "/commerce/price"]),
    column("STOCK", &["/stock", "/commerce/stock"]),
    column("ACTIVE", &["/active"]),
];

const ORDER_COLUMNS: &[Column] = &[
    column("ID", &["/id"]),
    column("NUMBER", &["/orderNumber", "/number"]),
    column("CREATED", &["/createdAt"]),
    column("STATE", &["/state", "/status"]),
    column("TOTAL", &["/total", "/amountTotal", "/price/totalPrice"]),
    column("CUSTOMER", &["/customerEmail"]),
];

/// Reads cursor pages (`after` / `nextCursor`) until `limit` rows or the last page.
async fn pages<F, Fut>(limit: usize, mut fetch: F) -> Outcome<Vec<Value>>
where
    F: FnMut(Vec<(String, String)>) -> Fut,
    Fut: Future<Output = Result<Value, veyndo_sdk::Error>>,
{
    let mut rows = Vec::new();
    let mut after: Option<String> = None;
    while rows.len() < limit {
        let mut query = vec![(
            "limit".to_string(),
            (limit - rows.len()).min(100).to_string(),
        )];
        if let Some(after) = after.take() {
            query.push(("after".into(), after));
        }
        let page = fetch(query).await?;
        rows.extend(page["elements"].as_array().cloned().unwrap_or_default());
        after = page["nextCursor"].as_str().map(String::from);
        if after.is_none() || page["hasMore"] == json!(false) {
            break;
        }
    }
    rows.truncate(limit);
    Ok(rows)
}

fn scoped(run: &Run, sandbox: Option<&str>) -> Outcome<Client> {
    match sandbox {
        Some(sandbox) => run.context.client_for(sandbox),
        None => run.context.client(),
    }
}

/// `veyndo products …`
pub async fn products(run: &Run, command: ProductsCommand) -> Outcome {
    match command {
        ProductsCommand::List {
            search,
            sandbox,
            limit,
        } => {
            let client = scoped(run, sandbox.as_deref())?;
            let rows = pages(limit, |mut query| {
                if let Some(search) = &search {
                    query.push(("search".into(), search.clone()));
                }
                let client = &client;
                async move {
                    client
                        .catalog()
                        .get_merchant_products(RequestOptions {
                            query,
                            ..RequestOptions::default()
                        })
                        .await
                }
            })
            .await?;
            output::rows(
                run.format,
                &json!({"elements": rows}),
                &rows,
                PRODUCT_COLUMNS,
            );
            Ok(())
        }
        ProductsCommand::Get { id, sandbox } => {
            let product = scoped(run, sandbox.as_deref())?
                .catalog()
                .get_merchant_products_by_id(&id, RequestOptions::default())
                .await?;
            detail(run, &product, PRODUCT_COLUMNS);
            Ok(())
        }
        ProductsCommand::Import {
            file,
            sandbox,
            tax_rate,
            format,
        } => import(run, &file, sandbox, tax_rate, format).await,
    }
}

/// `veyndo orders …`
pub async fn orders(run: &Run, command: OrdersCommand) -> Outcome {
    let client = run.context.client()?;
    match command {
        OrdersCommand::List {
            search,
            state,
            limit,
        } => {
            let rows = pages(limit, |mut query| {
                if let Some(search) = &search {
                    query.push(("query".into(), search.clone()));
                }
                if let Some(state) = &state {
                    query.push(("state".into(), state.clone()));
                }
                let client = &client;
                async move {
                    client
                        .orders()
                        .get_merchant_orders(RequestOptions {
                            query,
                            ..RequestOptions::default()
                        })
                        .await
                }
            })
            .await?;
            output::rows(run.format, &json!({"elements": rows}), &rows, ORDER_COLUMNS);
            Ok(())
        }
        OrdersCommand::Get { id } => {
            let order = client
                .orders()
                .get_merchant_orders_by_id(&id, RequestOptions::default())
                .await?;
            detail(run, &order, ORDER_COLUMNS);
            Ok(())
        }
    }
}

/// One record: the summary columns as fields, or the complete JSON.
fn detail(run: &Run, value: &Value, columns: &[Column]) {
    let lines: Vec<(&str, String)> = columns
        .iter()
        .map(|c| {
            let found = c
                .pointers
                .iter()
                .filter_map(|p| value.pointer(p))
                .find(|v| !v.is_null());
            (c.header, output::cell(found))
        })
        .collect();
    output::fields(run.format, value, &lines);
    if !run.json() {
        output::note("Use --json for the complete record.");
    }
}

fn read_source(file: &Path) -> Outcome<String> {
    let size = std::fs::metadata(file)
        .map_err(|e| Failure::invalid(format!("Cannot read {}: {e}", file.display())))?
        .len();
    if size > SOURCE_LIMIT {
        return Err(Failure::invalid(format!(
            "{} is {size} bytes; an import batch takes at most {SOURCE_LIMIT} bytes (50 rows)",
            file.display()
        )));
    }
    let bytes = std::fs::read(file)?;
    let text = String::from_utf8(bytes)
        .map_err(|_| Failure::invalid(format!("{} is not UTF-8", file.display())))?;
    Ok(text.trim_start_matches('\u{feff}').to_string())
}

/// A stable product UUID per sandbox and draft, so a repeated import replays instead of duplicating.
fn draft_id(sandbox: &str, draft: &Value) -> String {
    let digest = Sha256::digest(format!("veyndo-cli-import\n{sandbox}\n{draft}").as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// The tax rate: the explicit one, or the shop's single configured rate.
fn tax_rate(explicit: Option<f64>, context: &Value) -> Outcome<f64> {
    if let Some(rate) = explicit {
        return if (0.0..=50.0).contains(&rate) {
            Ok(rate)
        } else {
            Err(Failure::invalid(
                "--tax-rate must be between 0 and 50 percent",
            ))
        };
    }
    let rates: Vec<f64> = context["taxes"]
        .as_array()
        .map(|taxes| {
            taxes
                .iter()
                .filter_map(|t| t["defaultRate"].as_f64())
                .collect()
        })
        .unwrap_or_default();
    match rates.as_slice() {
        [rate] => Ok(*rate),
        [] => Err(Failure::invalid(
            "The shop has no tax rate configured; pass --tax-rate",
        )),
        _ => Err(Failure::invalid(format!(
            "The shop has several tax rates ({rates:?}); choose one with --tax-rate"
        ))),
    }
}

async fn import(
    run: &Run,
    file: &Path,
    sandbox: Option<String>,
    explicit_tax: Option<f64>,
    format: CsvFormat,
) -> Outcome {
    let source = read_source(file)?;
    let sandbox = match sandbox {
        Some(sandbox) => {
            sandbox::known(run, &sandbox).await?;
            sandbox
        }
        None => {
            let name = format!(
                "Import {}",
                file.file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
            );
            let id = sandbox::create(run, &name).await?;
            output::note(&format!("Created sandbox {id}"));
            id
        }
    };
    let client = run.context.client_for(&sandbox)?;
    let intake = client
        .catalog()
        .get_merchant_catalog_intake(RequestOptions::default())
        .await?;
    let rate = tax_rate(explicit_tax, &intake)?;
    let mode = match format {
        CsvFormat::Veyndo => "csv",
        CsvFormat::Shopify => "shopify_csv",
    };
    let prepared = client
        .catalog()
        .post_merchant_catalog_intake(RequestOptions::json(
            json!({"mode": mode, "source": source, "count": 10}),
        ))
        .await?;
    let drafts = prepared["drafts"].as_array().cloned().unwrap_or_default();
    let mut results = Vec::new();
    let mut first_failure: Option<Failure> = None;
    for draft in &drafts {
        let id = draft_id(&sandbox, draft);
        let saved = client
            .catalog()
            .post_merchant_catalog_intake_save(RequestOptions::json(json!({
                "id": id, "approve": true, "settingsRevision": prepared["settingsRevision"],
                "mainLocale": prepared["mainLocale"], "currency": prepared["currency"],
                "taxRate": rate, "demo": prepared["demo"], "draft": draft,
            })))
            .await;
        let (status, problem) = match saved {
            Ok(value) if value["replayed"] == json!(true) => ("unchanged", None),
            Ok(_) => ("created", None),
            Err(error) => {
                let failure = Failure::from(error);
                let message = failure.message.clone();
                first_failure.get_or_insert(failure);
                ("failed", Some(message))
            }
        };
        results.push(json!({"id": id, "name": draft["name"], "price": draft["price"], "status": status, "problem": problem}));
    }
    let summary = json!({"sandbox": sandbox, "currency": prepared["currency"], "taxRate": rate, "products": results});
    output::rows(
        run.format,
        &summary,
        &results,
        &[
            column("STATUS", &["/status"]),
            column("ID", &["/id"]),
            column("NAME", &["/name"]),
            column("PRICE", &["/price"]),
            column("PROBLEM", &["/problem"]),
        ],
    );
    if let Some(failure) = first_failure {
        return Err(Failure::new(
            failure.exit,
            format!("Some rows were not imported: {}", failure.message),
        ));
    }
    if drafts.is_empty() {
        return Err(Failure::new(Exit::Invalid, "The CSV contained no products"));
    }
    if !run.json() {
        sandbox::next_steps(&sandbox);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_ids_are_stable_uuids_per_sandbox() {
        let draft = json!({"name":"Lamp","price":12.5});
        let a = draft_id("shop-stage-1", &draft);
        assert_eq!(a, draft_id("shop-stage-1", &draft));
        assert_ne!(a, draft_id("shop-stage-2", &draft));
        assert_eq!(a.len(), 36);
        assert_eq!(&a[14..15], "5");
        assert!(matches!(&a[19..20], "8" | "9" | "a" | "b"));
    }

    #[test]
    fn tax_rate_needs_one_unambiguous_rate() {
        assert_eq!(
            tax_rate(None, &json!({"taxes":[{"defaultRate":19.0}]})).unwrap(),
            19.0
        );
        assert_eq!(tax_rate(Some(7.0), &json!({})).unwrap(), 7.0);
        assert!(
            tax_rate(
                None,
                &json!({"taxes":[{"defaultRate":19.0},{"defaultRate":7.0}]})
            )
            .is_err()
        );
        assert!(tax_rate(None, &json!({"taxes":[]})).is_err());
        assert!(tax_rate(Some(80.0), &json!({})).is_err());
    }
}
