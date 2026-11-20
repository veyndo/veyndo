//! Which declared actions only read: the distinction every surface, grant and GET route relies on.
use veyndo_app_model::Action;
use veyndo_kernel::verified_kernel;

/// Whether an action only reads; a writing handler never counts, whatever the package declares.
pub fn readonly(action: &Action) -> bool {
    verified_kernel::app_read_admissible(
        action.read_only || ["list", "configurations"].contains(&action.handler.as_str()),
        ["save", "emit", "submit"].contains(&action.handler.as_str()),
    )
}

#[cfg(test)]
mod tests {
    use super::readonly;
    use crate::domain::approval::built_in;
    use veyndo_app_model::manifest::Manifest;

    #[test]
    fn listing_reads_and_a_writing_handler_never_does() {
        let m: Manifest = serde_json::from_str(built_in("engraving").unwrap()).unwrap();
        let mut action = m.actions[0].clone();
        for (handler, declared, expected) in [
            ("list", false, true),
            ("configurations", false, true),
            ("save", true, false),
            ("emit", true, false),
            ("submit", false, false),
            ("service", true, true),
            ("service", false, false),
        ] {
            action.handler = handler.into();
            action.read_only = declared;
            assert_eq!(readonly(&action), expected, "{handler} {declared}");
        }
    }
}
