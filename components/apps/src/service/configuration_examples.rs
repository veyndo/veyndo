//! Frozen configuration cases use the same field validation and bounded guest as real cart configuration.
use crate::domain::configuration_inputs;
use crate::service::configuration_runtime as runtime;
use veyndo_app_model::configuration::Contract;
use veyndo_core::{Error, Result};

/// Runs every frozen example of a contract and requires its recorded result.
pub async fn verify(c: &Contract) -> Result<()> {
    let Some(inputs) = &c.inputs else {
        return Ok(());
    };
    let guest = runtime::prepare(c).await?;
    for (index, example) in inputs.examples.iter().enumerate() {
        let actual = match configuration_inputs::arguments(c, &example.fields) {
            Ok(_) => runtime::configured_contribution(
                guest.clone(),
                c,
                example.material_fee_minor,
                &example.fields,
            )
            .await
            .ok(),
            Err(_) => None,
        };
        if actual != example.expected_fee_minor {
            return Err(Error::validation(
                &format!("/configuration/inputs/examples/{index}/expectedFeeMinor"),
                format!(
                    "Configuration example {} does not match its frozen expected result",
                    example.id
                ),
            ));
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::verify;
    use veyndo_app_model::manifest::Manifest;
    #[tokio::test]
    async fn configuration_examples_fence_real_guest_changes_and_rejections() {
        let m: Manifest = serde_json::from_str(include_str!(
            "../../../../extensions/apps/furniture-configurator/manifest.json"
        ))
        .unwrap();
        let c = m.configuration.unwrap();
        verify(&c).await.unwrap();
        let mut changed = c.clone();
        changed.wasm_source = changed
            .wasm_source
            .replace("i64.const 4800", "i64.const 5800");
        assert_eq!(
            verify(&changed).await.unwrap_err().problem()["pointer"],
            "/configuration/inputs/examples/1/expectedFeeMinor"
        );
        let mut wrong = c;
        wrong.inputs.as_mut().unwrap().examples[0].expected_fee_minor = Some(1);
        assert!(verify(&wrong).await.is_err());
    }
}
