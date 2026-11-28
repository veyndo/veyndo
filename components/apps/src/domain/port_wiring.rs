//! Typed app-to-app ports: a consumer names an interface and version range, the platform picks the installed provider; incompatible versions are refused before installation, upgrade, rollback or deactivation.
use crate::domain::package_contract::{ConsumedPort, Contract, ProvidedPort};
use veyndo_core::{Result, conflict};

/// The active contract of one installed app.
#[derive(Clone, Copy)]
pub struct Installed<'a> {
    pub app: &'a str,
    pub contract: &'a Contract,
}
/// The provider a consumed port resolves to: the highest compatible version, then the lowest app ID,
/// never the consumer itself.
pub fn resolve<'a>(
    consumer: &str,
    port: &ConsumedPort,
    installed: &'a [Installed<'a>],
) -> Option<(&'a str, &'a ProvidedPort)> {
    let range = semver::VersionReq::parse(&port.version).ok()?;
    installed
        .iter()
        .filter(|i| i.app != consumer)
        .flat_map(|i| i.contract.provides.iter().map(move |p| (i.app, p)))
        .filter_map(|(app, p)| {
            let v = semver::Version::parse(&p.version).ok()?;
            (p.name == port.name && range.matches(&v)).then_some((v, app, p))
        })
        .max_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.cmp(a.1)))
        .map(|(_, app, p)| (app, p))
}
/// Admits `contract` for `app` against the other active apps: every consumed port must resolve, and
/// no other app may lose the provider it resolves to today.
pub fn admit(app: &str, contract: &Contract, others: &[Installed<'_>]) -> Result<()> {
    let others: Vec<Installed<'_>> = others.iter().copied().filter(|i| i.app != app).collect();
    for port in &contract.consumes {
        if resolve(app, port, &others).is_none() {
            return Err(conflict(&format!(
                "No installed app provides port {} in a version matching {}",
                port.name, port.version
            )));
        }
    }
    let mut after = others.clone();
    after.push(Installed { app, contract });
    for consumer in &others {
        for port in &consumer.contract.consumes {
            if resolve(consumer.app, port, &after).is_none() {
                return Err(conflict(&format!(
                    "This version would leave {} without a compatible provider for port {}",
                    consumer.app, port.name
                )));
            }
        }
    }
    Ok(())
}
/// Admits removing `app` (deactivation): no remaining app may lose its provider.
pub fn admit_removal(app: &str, installed: &[Installed<'_>]) -> Result<()> {
    let rest: Vec<Installed<'_>> = installed.iter().copied().filter(|i| i.app != app).collect();
    for consumer in &rest {
        for port in &consumer.contract.consumes {
            if resolve(consumer.app, port, &rest).is_none() {
                return Err(conflict(&format!(
                    "{} still consumes port {} from this app",
                    consumer.app, port.name
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn provider(app: &str, version: &str) -> Contract {
        Contract {
            app: app.into(),
            version: "1.0.0".into(),
            provides: vec![ProvidedPort {
                name: "acme.quote".into(),
                version: version.into(),
                action: "quote".into(),
                input_schema: json!({"type":"object"}),
                output_schema: json!({"type":"object"}),
            }],
            ..Contract::default()
        }
    }
    fn consumer(range: &str) -> Contract {
        Contract {
            app: "shop_ui".into(),
            version: "1.0.0".into(),
            consumes: vec![ConsumedPort {
                name: "acme.quote".into(),
                version: range.into(),
            }],
            ..Contract::default()
        }
    }

    #[test]
    fn the_highest_compatible_provider_is_wired_and_an_incompatible_one_is_refused() {
        let (a, b) = (provider("acme_a", "1.2.0"), provider("acme_b", "1.4.1"));
        let installed = [
            Installed {
                app: "acme_a",
                contract: &a,
            },
            Installed {
                app: "acme_b",
                contract: &b,
            },
        ];
        let c = consumer("^1.1");
        let (app, port) = resolve("shop_ui", &c.consumes[0], &installed).unwrap();
        assert_eq!((app, port.version.as_str()), ("acme_b", "1.4.1"));
        admit("shop_ui", &c, &installed).unwrap();
        let too_new = consumer("^2");
        assert!(admit("shop_ui", &too_new, &installed).is_err());
        assert!(admit("shop_ui", &c, &[]).is_err());
    }

    #[test]
    fn an_upgrade_or_removal_that_strands_a_consumer_is_refused() {
        let old = provider("acme_a", "1.2.0");
        let c = consumer("^1.0");
        let installed = [
            Installed {
                app: "acme_a",
                contract: &old,
            },
            Installed {
                app: "shop_ui",
                contract: &c,
            },
        ];
        admit("acme_a", &provider("acme_a", "1.3.0"), &installed).unwrap();
        let err = admit("acme_a", &provider("acme_a", "2.0.0"), &installed).unwrap_err();
        assert_eq!(err.0, veyndo_core::StatusCode::CONFLICT);
        assert!(admit_removal("acme_a", &installed).is_err());
        assert!(admit_removal("shop_ui", &installed).is_ok());
        let other = provider("acme_b", "1.0.5");
        let both = [
            installed[0],
            installed[1],
            Installed {
                app: "acme_b",
                contract: &other,
            },
        ];
        admit("acme_a", &provider("acme_a", "2.0.0"), &both).unwrap();
        admit_removal("acme_a", &both).unwrap();
    }
}
