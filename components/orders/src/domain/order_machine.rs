//! Declarative order workflow schema. Extensions add states, never executable effects or payment truth.
use std::collections::HashMap;
use veyndo_app_model::declarations::identifier;
pub use veyndo_app_model::native_lifecycle::{Machine as OrderMachine, MachineEdge, MachineState};
use veyndo_core::{Result, bad};
fn labels(w: [&str; 4]) -> HashMap<String, String> {
    ["en", "de", "fr", "es"]
        .into_iter()
        .zip(w)
        .map(|(l, v)| (l.into(), v.into()))
        .collect()
}
pub fn default_machine() -> OrderMachine {
    let states = [
        (
            "placed",
            ["Received", "Eingegangen", "Reçue", "Recibido"],
            false,
        ),
        (
            "in_progress",
            ["In progress", "In Bearbeitung", "En cours", "En proceso"],
            false,
        ),
        (
            "completed",
            ["Completed", "Abgeschlossen", "Terminée", "Completado"],
            true,
        ),
        (
            "cancelled",
            ["Cancelled", "Storniert", "Annulée", "Cancelado"],
            true,
        ),
    ]
    .into_iter()
    .map(|(id, w, terminal)| MachineState {
        id: id.into(),
        label: labels(w),
        terminal,
    })
    .collect();
    let transitions = [
        (
            "start",
            "placed",
            "in_progress",
            [
                "Start processing",
                "Bearbeitung starten",
                "Commencer le traitement",
                "Iniciar gestión",
            ],
        ),
        (
            "complete",
            "in_progress",
            "completed",
            [
                "Complete order",
                "Bestellung abschließen",
                "Terminer la commande",
                "Completar pedido",
            ],
        ),
        (
            "cancel_new",
            "placed",
            "cancelled",
            [
                "Cancel order",
                "Bestellung stornieren",
                "Annuler la commande",
                "Cancelar pedido",
            ],
        ),
        (
            "cancel_active",
            "in_progress",
            "cancelled",
            [
                "Cancel order",
                "Bestellung stornieren",
                "Annuler la commande",
                "Cancelar pedido",
            ],
        ),
    ]
    .into_iter()
    .map(|(id, from, to, w)| MachineEdge {
        id: id.into(),
        from: from.into(),
        to: to.into(),
        label: labels(w),
        policy: None,
    })
    .collect();
    OrderMachine {
        states,
        transitions,
        source_app: None,
    }
}
pub trait OrderMachineValidation {
    fn validate(&self) -> Result<()>;
}
impl OrderMachineValidation for OrderMachine {
    fn validate(&self) -> Result<()> {
        if self.states.len() > 32 || self.transitions.len() > 100 {
            return Err(bad("Workflow exceeds limits"));
        }
        let mut ids = std::collections::HashSet::new();
        let valid_labels = |v: &HashMap<String, String>| {
            ["en", "de", "fr", "es"].iter().all(|l| {
                v.get(*l)
                    .is_some_and(|s| !s.trim().is_empty() && s.len() <= 100)
            })
        };
        for s in &self.states {
            if !identifier(&s.id) || !valid_labels(&s.label) || !ids.insert(s.id.clone()) {
                return Err(bad("Invalid workflow state"));
            }
        }
        for id in ["placed", "in_progress", "completed", "cancelled"] {
            let s = self
                .states
                .iter()
                .find(|s| s.id == id)
                .ok_or(bad("Required workflow state missing"))?;
            if s.terminal != ["completed", "cancelled"].contains(&id) {
                return Err(bad("Built-in terminal states cannot be changed"));
            }
        }
        let mut edges = std::collections::HashSet::new();
        let mut pairs = std::collections::HashSet::new();
        for e in &self.transitions {
            if !identifier(&e.id)
                || !valid_labels(&e.label)
                || !ids.contains(&e.from)
                || !ids.contains(&e.to)
                || e.from == e.to
                || !edges.insert(e.id.clone())
                || !pairs.insert((&e.from, &e.to))
                || self.states.iter().any(|s| s.id == e.from && s.terminal)
            {
                return Err(bad("Invalid workflow transition"));
            }
        }
        // Every added state must be reachable from placed; hidden dead paths cannot strand new orders.
        let mut reachable = std::collections::HashSet::from(["placed".to_string()]);
        for _ in 0..self.states.len() {
            for e in &self.transitions {
                if reachable.contains(&e.from) {
                    reachable.insert(e.to.clone());
                }
            }
        }
        if self.states.iter().any(|s| !reachable.contains(&s.id)) {
            return Err(bad("Unreachable workflow state"));
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn terminal_and_executable_edges_rejected() {
        let mut m = default_machine();
        assert!(m.validate().is_ok());
        m.transitions[0].from = "completed".into();
        assert!(m.validate().is_err());
        assert!(
            serde_json::from_value::<MachineEdge>(
                json!({"id":"run","from":"placed","to":"completed","label":{},"shell":"exec"})
            )
            .is_err()
        );
    }
}
