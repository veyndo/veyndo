//! Typed, bounded private preference graph: no dangling relations and no free-form kinds.
use serde::{Deserialize, Serialize};
use veyndo_core::{Result, bad};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// One private fact about the shopper.
pub struct Node {
    pub id: String,
    pub kind: String,
    pub value: String,
    pub product_id: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// A typed relation between two facts.
pub struct Edge {
    pub source: String,
    pub target: String,
    pub kind: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// The bounded private preference graph of one cart.
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}
impl Graph {
    pub fn validate(&self) -> Result<()> {
        if self.nodes.len() > 32 || self.edges.len() > 64 {
            return Err(bad("Preference graph limit: 32 nodes and 64 edges"));
        }
        let mut ids = std::collections::HashSet::new();
        for n in &self.nodes {
            if n.id.is_empty()
                || n.id.len() > 64
                || !n
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                || !ids.insert(&n.id)
                || !["size", "style", "owned_product", "interest"].contains(&n.kind.as_str())
                || n.value.is_empty()
                || n.value.len() > 240
                || (n.kind == "owned_product" && n.product_id.is_none())
            {
                return Err(bad("Invalid private preference node"));
            }
        }
        if self.edges.iter().any(|e| {
            !ids.contains(&e.source)
                || !ids.contains(&e.target)
                || !["PREFERS", "FITS", "COMPLEMENTS"].contains(&e.kind.as_str())
        }) {
            return Err(bad("Invalid preference relation"));
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::{Edge, Graph, Node};
    #[test]
    fn private_graph_has_typed_bounded_non_dangling_relations() {
        let mut g = Graph {
            nodes: vec![Node {
                id: "size".into(),
                kind: "size".into(),
                value: "M".into(),
                product_id: None,
            }],
            edges: vec![],
        };
        assert!(g.validate().is_ok());
        g.edges.push(Edge {
            source: "size".into(),
            target: "foreign".into(),
            kind: "FITS".into(),
        });
        assert!(g.validate().is_err());
        g.edges.clear();
        g.nodes[0].kind = "payment_token".into();
        assert!(g.validate().is_err());
    }
}
