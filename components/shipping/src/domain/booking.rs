//! A carrier booking occupies one open package; only the intent that reserved it can release it.
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{merge, view};
use veyndo_core::{Result, conflict};
use veyndo_pricing::verified_kernel;

pub enum Step {
    /// Nothing to write; answer with this.
    Settled(Value),
    /// The package changed; record this event.
    Changed(&'static str),
}

/// Reserves or releases the package for `intent`. `closed` says the order's delivery no longer accepts bookings.
/// The facts of a package a carrier booking depends on.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ParcelBooking {
    state: Value,
    label: Value,
    booking: Value,
}

pub fn transition(parcel: &mut Value, reserve: bool, closed: bool, intent: &Value) -> Result<Step> {
    let current: ParcelBooking = view(parcel);
    if !verified_kernel::carrier_booking_admissible(
        current.state == "open",
        closed,
        !current.label.is_null(),
        !current.booking.is_null(),
        current.booking == *intent,
    ) {
        return Err(conflict(
            "Package is closed, labelled or held by another carrier booking",
        ));
    }
    if reserve {
        if current.booking == *intent {
            return Ok(Step::Settled(json!({"reserved":true,"reused":true})));
        }
        if !current.booking.is_null() {
            return Err(conflict(
                "Another unresolved carrier booking occupies this package",
            ));
        }
        merge(parcel, json!({"booking": intent}))?;
        Ok(Step::Changed("delivery.booking_reserved"))
    } else {
        if current.booking.is_null() {
            return Ok(Step::Settled(json!({"released":true})));
        }
        if current.booking != *intent {
            return Err(conflict("Only the original carrier intent can be released"));
        }
        if let Some(fields) = parcel.as_object_mut() {
            fields.remove("booking");
        }
        Ok(Step::Changed("delivery.booking_released"))
    }
}

#[cfg(test)]
mod tests {
    use super::{Step, transition};
    use serde_json::json;

    #[test]
    fn one_intent_holds_a_package_until_it_releases_it() {
        let mine = json!({"sourceApp":"carrier","requestKey":"request-1"});
        let other = json!({"sourceApp":"carrier","requestKey":"request-2"});
        let mut parcel = json!({"state":"open"});
        assert!(matches!(
            transition(&mut parcel, true, false, &mine),
            Ok(Step::Changed("delivery.booking_reserved"))
        ));
        assert!(matches!(
            transition(&mut parcel, true, false, &mine),
            Ok(Step::Settled(_))
        ));
        assert!(transition(&mut parcel, true, false, &other).is_err());
        assert!(transition(&mut parcel, false, false, &other).is_err());
        assert!(matches!(
            transition(&mut parcel, false, false, &mine),
            Ok(Step::Changed("delivery.booking_released"))
        ));
        assert!(parcel.get("booking").is_none());
    }

    #[test]
    fn closed_or_labelled_packages_admit_no_new_booking() {
        let intent = json!({"sourceApp":"carrier","requestKey":"request-1"});
        assert!(transition(&mut json!({"state":"open"}), true, true, &intent).is_err());
        assert!(transition(&mut json!({"state":"shipped"}), true, false, &intent).is_err());
        assert!(
            transition(
                &mut json!({"state":"open","label":{"id":"l"}}),
                true,
                false,
                &intent
            )
            .is_err()
        );
    }
}
