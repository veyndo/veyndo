//! A review as a shopper submits it and as a migration imports it; both arrive unpublished.
use chrono::{DateTime, FixedOffset};
use serde_json::Value;
use veyndo_core::{Result, bad};

/// A review written in the storefront.
#[derive(Debug, PartialEq)]
pub struct NewReview {
    pub author: String,
    pub title: String,
    pub content: String,
    pub rating: i32,
}

impl NewReview {
    pub fn parse(v: &Value) -> Result<Self> {
        let rating = v["rating"]
            .as_i64()
            .filter(|v| (1..=5).contains(v))
            .ok_or(bad("Rating must be 1..5"))?;
        let text = |key: &str, max: usize| -> Result<String> {
            let s = v[key].as_str().unwrap_or("").trim();
            if s.is_empty() || s.chars().count() > max {
                return Err(bad(format!("Invalid {key}")));
            }
            Ok(s.into())
        };
        Ok(Self {
            author: text("author", 80)?,
            title: text("title", 120)?,
            content: text("content", 3000)?,
            rating: rating as i32,
        })
    }
}

/// A historical review from another system; it never counts as a verified purchase.
#[derive(Debug)]
pub struct ImportedReview<'a> {
    pub author: &'a str,
    pub title: &'a str,
    pub content: &'a str,
    pub rating: i32,
    pub created: DateTime<FixedOffset>,
}

impl<'a> ImportedReview<'a> {
    pub fn parse(data: &'a Value) -> Result<Self> {
        let field = |key: &str, max: usize| -> Result<&'a str> {
            data[key]
                .as_str()
                .filter(|s| !s.trim().is_empty() && s.chars().count() <= max)
                .ok_or(bad(format!("Original review {key} required")))
        };
        let author = field("sourceAuthor", 80)?;
        let title = field("title", 120)?;
        let content = field("text", 3000)?;
        let rating = data["rating"]
            .as_i64()
            .filter(|n| (1..=5).contains(n))
            .ok_or(bad("Review rating must be 1..5"))?;
        let created = field("createdAt", 100).and_then(|s| {
            DateTime::parse_from_rfc3339(s)
                .map_err(|_| bad("Original review date requires an explicit timezone"))
        })?;
        if data["verifiedPurchase"] != false {
            return Err(bad(
                "Imported purchase claims are not native verified-purchase evidence",
            ));
        }
        Ok(Self {
            author,
            title,
            content,
            rating: rating as i32,
            created,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{ImportedReview, NewReview};
    use serde_json::json;

    #[test]
    fn a_submitted_review_is_trimmed_and_bounded() {
        let review = NewReview::parse(
            &json!({"rating":5,"author":" Ada ","title":"Good","content":"Works"}),
        )
        .unwrap();
        assert_eq!(review.author, "Ada");
        assert_eq!(review.rating, 5);
        for broken in [
            json!({"rating":0,"author":"A","title":"T","content":"C"}),
            json!({"rating":6,"author":"A","title":"T","content":"C"}),
            json!({"rating":3,"author":"  ","title":"T","content":"C"}),
            json!({"rating":3,"author":"A","title":"T".repeat(121),"content":"C"}),
            json!({"rating":3,"author":"A","title":"T"}),
        ] {
            assert!(NewReview::parse(&broken).is_err(), "{broken}");
        }
    }

    #[test]
    fn an_imported_review_needs_a_timezone_and_never_claims_a_purchase() {
        let base = json!({"sourceAuthor":"Ada","title":"Good","text":"Works","rating":4,
            "createdAt":"2024-05-01T10:00:00+02:00","verifiedPurchase":false});
        let review = ImportedReview::parse(&base).unwrap();
        assert_eq!(review.created.to_rfc3339(), "2024-05-01T10:00:00+02:00");
        for (key, value) in [
            ("createdAt", json!("2024-05-01T10:00:00")),
            ("verifiedPurchase", json!(true)),
            ("verifiedPurchase", json!(null)),
            ("rating", json!(9)),
            ("sourceAuthor", json!("")),
        ] {
            let mut broken = base.clone();
            broken[key] = value;
            assert!(ImportedReview::parse(&broken).is_err(), "{key}");
        }
    }
}
