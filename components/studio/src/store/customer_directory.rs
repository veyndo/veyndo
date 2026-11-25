//! SQL of the merchant CRM directory: one keyset page of customer contacts with optional filters and app extension predicates.
use serde_json::Value;
use veyndo_app_model::Object;
use veyndo_app_platform::domain::extension_filter::Filter;
use veyndo_app_platform::store::extension_filter::predicate;
use veyndo_core::{Result, State, bad};
use veyndo_customers::store::customers::Customer;
use veyndo_orders::types::list_criteria::Criteria;

/// One page (`n + 1` rows) of contacts; the default email order without filters takes the plain prepared query.
pub async fn page(
    a: &State,
    t: &str,
    c: Criteria,
    (sort, direction): (&str, &str),
    filters: &[Filter],
    n: i64,
) -> Result<Vec<Value>> {
    Ok(
        if filters.is_empty()
            && (c.sort.is_empty() || c.sort == "email")
            && direction == "ASC"
            && c.guest.is_none()
            && c.active.is_none()
            && c.group.is_empty()
        {
            sqlx::query_file_scalar!(
                "src/store/customer_directory_default.sql",
                t,
                c.after,
                c.query,
                n + 1
            )
            .fetch_all(&a.db)
            .await?
        } else {
            use sqlx::Arguments;
            let mut arguments = sqlx::postgres::PgArguments::default();
            arguments
                .add(t.to_owned())
                .map_err(|_| bad("Invalid tenant binding"))?;
            // dynamic SQL: admitted sort column, optional filters and app extension predicates.
            let mut q = sqlx::QueryBuilder::<sqlx::Postgres>::with_arguments(
                include_str!("customer_directory.sql"),
                arguments,
            );
            if !c.after.is_empty() {
                q.push(format!(
                    " AND ({sort},email) {} (SELECT {sort},email FROM contacts WHERE email=",
                    if direction == "ASC" { ">" } else { "<" }
                ))
                .push_bind(c.after)
                .push(")");
            }
            if !c.query.is_empty() {
                q.push(" AND strpos(lower(email||' '||coalesce(contact->'profile'->>'name','')||' '||coalesce(contact->>'company','')),lower(").push_bind(c.query).push("))>0");
            }
            if let Some(guest) = c.guest {
                q.push(" AND (contact->>'guest')::bool=").push_bind(guest);
            }
            if let Some(active) = c.active {
                q.push(" AND (contact->>'active')::bool=").push_bind(active);
            }
            if !c.group.is_empty() {
                q.push(" AND contact->>'customerGroup'=").push_bind(c.group);
            }
            predicate(&mut q, Object::Customer, t, filters);
            q.push(format!(
                " ORDER BY {sort} {direction},email {direction} LIMIT "
            ))
            .push_bind(n + 1);
            q.build_query_scalar::<Value>().fetch_all(&a.db).await?
        },
    )
}

/// A customer by email with its creation time; `None` for a guest.
pub async fn customer(a: &State, t: &str, email: &str) -> Result<Option<Customer>> {
    veyndo_customers::store::manual_orders::find_by_email(a, t, email).await
}

/// The customer's 100 most recent orders (without cart tokens), each with `createdAt`.
pub async fn orders(a: &State, t: &str, email: &str) -> Result<Vec<Value>> {
    veyndo_customers::store::manual_orders::orders(a, t, email).await
}
