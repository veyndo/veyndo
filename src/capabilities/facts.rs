//! The typed facts of every native operation, keyed by its catalogue name; the registry refuses an operation without them.
//!
//! Permissions are the exact set current admission requires (`tests::declared_permissions_match_admission`
//! proves both sufficiency and necessity). Reversibility, risk, data and side effects are reviewed
//! declarations; `OperationFacts::check` rejects contradictory ones and the registry rejects
//! compensations or requirements that name no operation.
use veyndo_core::operation_facts::{
    Audience, Condition as C, Entity as E, OperationFacts as F, Reversibility, Risk,
    SideEffect as S,
};

pub(super) const FACTS: &[(&str, F)] = &[
    (
        "currency.list",
        F::read(Audience::Shopper, &[]).on(&[E::Currency]).money(),
    ),
    (
        "currency.select",
        F::write(Audience::Shopper, &[], Reversibility::Reversible, Risk::Low)
            .on(&[E::Cart, E::Currency])
            .money(),
    ),
    (
        "merchant.currencies.refresh",
        F::write(
            Audience::Merchant,
            &["settings.write"],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::Currency])
        .money()
        .effects(&[S::ExternalCall]),
    ),
    (
        "merchant.currencies.generate",
        F::write(
            Audience::Merchant,
            &["catalog.write"],
            Reversibility::Reversible,
            Risk::Medium,
        )
        .on(&[E::Currency, E::Product])
        .money()
        .effects(&[S::BackgroundJob]),
    ),
    (
        "merchant.currencies.jobs",
        F::read(Audience::Merchant, &["catalog.read"]).on(&[E::Currency]),
    ),
    (
        "merchant.currencies.job",
        F::read(Audience::Merchant, &["catalog.read"])
            .on(&[E::Currency])
            .requires(&["merchant.currencies.generate"]),
    ),
    (
        "privacy.policy",
        F::read(Audience::Shopper, &[]).on(&[E::Legal]),
    ),
    (
        "privacy.consent",
        F::write(Audience::Shopper, &[], Reversibility::Reversible, Risk::Low)
            .on(&[E::Legal, E::Cart]),
    ),
    (
        "legal.accept",
        F::write(
            Audience::Shopper,
            &[],
            Reversibility::Irreversible,
            Risk::Medium,
        )
        .on(&[E::Legal, E::Cart]),
    ),
    (
        "legal.request",
        F::write(
            Audience::Shopper,
            &[],
            Reversibility::Irreversible,
            Risk::Medium,
        )
        .on(&[E::Legal, E::Order])
        .pii()
        .effects(&[S::Event]),
    ),
    (
        "merchant.legal.requests",
        F::read(Audience::Merchant, &["customers.read"])
            .on(&[E::Legal, E::Customer])
            .pii(),
    ),
    (
        "merchant.legal.review",
        F::write(
            Audience::Merchant,
            &["customers.write"],
            Reversibility::Irreversible,
            Risk::Medium,
        )
        .on(&[E::Legal, E::Customer])
        .pii()
        .effects(&[S::Event])
        .requires(&["merchant.legal.requests"]),
    ),
    (
        "merchant.history",
        F::read(Audience::Merchant, &["automation.read"]).on(&[E::History]),
    ),
    (
        "merchant.history.version",
        F::read(Audience::Merchant, &["automation.read"]).on(&[E::History]),
    ),
    (
        "merchant.history.restore",
        F::write(
            Audience::Merchant,
            &["automation.read", "automation.manage"],
            Reversibility::Reversible,
            Risk::Medium,
        )
        .on(&[
            E::History,
            E::Product,
            E::Category,
            E::Automation,
            E::Settings,
        ])
        .effects(&[S::Event])
        .requires(&["merchant.history.version"]),
    ),
    (
        "merchant.customer.groups",
        F::read(Audience::Merchant, &["customers.read"])
            .on(&[E::Customer, E::Settings])
            .money(),
    ),
    (
        "merchant.media.list",
        F::read(Audience::Merchant, &["catalog.read"]).on(&[E::Media]),
    ),
    (
        "merchant.commerce.dependencies",
        F::read(Audience::Merchant, &["settings.read"]).on(&[E::Settings]),
    ),
    (
        "merchant.media.provider",
        F::read(Audience::Merchant, &["catalog.read"]).on(&[E::Media]),
    ),
    (
        "merchant.media.create",
        F::write(
            Audience::Merchant,
            &["catalog.write"],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::Media])
        .effects(&[S::ModelCall, S::ExternalCall, S::BackgroundJob]),
    ),
    (
        "merchant.media.detail",
        F::read(Audience::Merchant, &["catalog.read"])
            .on(&[E::Media])
            .requires(&["merchant.media.create"]),
    ),
    (
        "merchant.media.apply",
        F::write(
            Audience::Merchant,
            &["catalog.write"],
            Reversibility::Reversible,
            Risk::Medium,
        )
        .on(&[E::Media, E::Product])
        .effects(&[S::Publication])
        .requires(&["merchant.media.create"]),
    ),
    (
        "merchant.company",
        F::read(Audience::Merchant, &["settings.read"]).on(&[E::Settings]),
    ),
    (
        "merchant.company.save",
        F::write(
            Audience::Merchant,
            &["settings.write"],
            Reversibility::Reversible,
            Risk::Medium,
        )
        .on(&[E::Settings])
        .effects(&[S::Publication]),
    ),
    (
        "merchant.commerce.read",
        F::read(Audience::Merchant, &["settings.read"])
            .on(&[E::Settings])
            .money(),
    ),
    (
        "merchant.commerce.save",
        F::write(
            Audience::Merchant,
            &["settings.write"],
            Reversibility::Reversible,
            Risk::High,
        )
        .on(&[E::Settings])
        .money()
        .effects(&[S::Publication]),
    ),
    (
        "merchant.translations.list",
        F::read(Audience::Merchant, &["catalog.read"]).on(&[E::Product, E::Category]),
    ),
    (
        "merchant.translations.create",
        F::write(
            Audience::Merchant,
            &["catalog.write"],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::Product, E::Category])
        .effects(&[S::ModelCall, S::BackgroundJob]),
    ),
    (
        "merchant.translations.detail",
        F::read(Audience::Merchant, &["catalog.read"])
            .on(&[E::Product, E::Category])
            .requires(&["merchant.translations.create"]),
    ),
    (
        "merchant.translations.control",
        F::write(
            Audience::Merchant,
            &["catalog.write"],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::Product])
        .effects(&[S::BackgroundJob])
        .requires(&["merchant.translations.create"]),
    ),
    (
        "merchant.translations.apply",
        F::write(
            Audience::Merchant,
            &["catalog.write"],
            Reversibility::Compensatable {
                by: "merchant.history.restore",
            },
            Risk::Medium,
        )
        .on(&[E::Product, E::Category])
        .effects(&[S::Publication, S::Event])
        .requires(&["merchant.translations.create"]),
    ),
    (
        "merchant.products",
        F::read(Audience::Merchant, &["catalog.read"])
            .on(&[E::Product])
            .money()
            .stock(),
    ),
    (
        "merchant.product.create",
        F::write(
            Audience::Merchant,
            &["catalog.write"],
            Reversibility::Compensatable {
                by: "merchant.product.save",
            },
            Risk::Medium,
        )
        .on(&[E::Product, E::Category])
        .money()
        .stock()
        .effects(&[S::Publication, S::Event]),
    ),
    (
        "merchant.categories",
        F::read(Audience::Merchant, &["catalog.read"]).on(&[E::Category]),
    ),
    (
        "merchant.category.create",
        F::write(
            Audience::Merchant,
            &["catalog.write"],
            Reversibility::Compensatable {
                by: "merchant.category.save",
            },
            Risk::Low,
        )
        .on(&[E::Category])
        .effects(&[S::Publication]),
    ),
    (
        "merchant.category.save",
        F::write(
            Audience::Merchant,
            &["catalog.write"],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::Category])
        .effects(&[S::Publication]),
    ),
    (
        "automation.catalog",
        F::read(Audience::Merchant, &["automation.read"]).on(&[E::Automation, E::App]),
    ),
    (
        "automation.list",
        F::read(Audience::Merchant, &["automation.read"]).on(&[E::Automation]),
    ),
    (
        "automation.save",
        F::write(
            Audience::Merchant,
            &["automation.manage"],
            Reversibility::Compensatable {
                by: "merchant.history.restore",
            },
            Risk::High,
        )
        .on(&[E::Automation])
        .money()
        .effects(&[S::Publication, S::Email]),
    ),
    (
        "automation.dependencies",
        F::read(Audience::Merchant, &["automation.read"]).on(&[E::Automation]),
    ),
    (
        "automation.delete",
        F::write(
            Audience::Merchant,
            &["automation.manage"],
            Reversibility::Irreversible,
            Risk::High,
        )
        .on(&[E::Automation])
        .requires(&["automation.dependencies"]),
    ),
    (
        "automation.preview",
        F::read(Audience::Merchant, &["automation.read"])
            .on(&[E::Automation, E::Cart])
            .money(),
    ),
    (
        "automation.import",
        F::read(Audience::Merchant, &["automation.manage"]).on(&[E::Automation]),
    ),
    (
        "merchant.workflow",
        F::read(Audience::Merchant, &["orders.read"]).on(&[E::Order, E::Settings]),
    ),
    (
        "merchant.workflow.save",
        F::write(
            Audience::Merchant,
            &["settings.write"],
            Reversibility::Reversible,
            Risk::Medium,
        )
        .on(&[E::Order, E::Settings]),
    ),
    (
        "merchant.product.content",
        F::read(Audience::Merchant, &["catalog.read"]).on(&[E::Product]),
    ),
    (
        "merchant.product.save",
        F::write(
            Audience::Merchant,
            &["catalog.write"],
            Reversibility::Compensatable {
                by: "merchant.history.restore",
            },
            Risk::Medium,
        )
        .on(&[E::Product])
        .money()
        .stock()
        .effects(&[S::Publication, S::Event])
        .requires(&["merchant.product.content"]),
    ),
    (
        "merchant.product.assets",
        F::read(Audience::Merchant, &["catalog.read"]).on(&[E::Media, E::Product]),
    ),
    (
        "merchant.asset.publish",
        F::write(
            Audience::Merchant,
            &["catalog.write"],
            Reversibility::Irreversible,
            Risk::Medium,
        )
        .on(&[E::Media, E::Product])
        .effects(&[S::Publication]),
    ),
    (
        "merchant.customer.addresses",
        F::read(Audience::Merchant, &["customers.read", "customers.pii"])
            .on(&[E::Customer])
            .pii(),
    ),
    (
        "merchant.customer.address.save",
        F::write(
            Audience::Merchant,
            &["customers.pii", "customers.write"],
            Reversibility::Reversible,
            Risk::Medium,
        )
        .on(&[E::Customer])
        .pii(),
    ),
    (
        "merchant.customer.address.delete",
        F::write(
            Audience::Merchant,
            &["customers.pii", "customers.write"],
            Reversibility::Irreversible,
            Risk::Medium,
        )
        .on(&[E::Customer])
        .pii()
        .requires(&["merchant.customer.addresses"]),
    ),
    (
        "merchant.customers",
        F::read(Audience::Merchant, &["customers.read", "customers.pii"])
            .on(&[E::Customer])
            .pii(),
    ),
    (
        "merchant.customer",
        F::read(Audience::Merchant, &["customers.read", "customers.pii"])
            .on(&[E::Customer, E::Order])
            .pii()
            .money(),
    ),
    (
        "merchant.customer.save",
        F::write(
            Audience::Merchant,
            &["customers.pii", "customers.write"],
            Reversibility::Reversible,
            Risk::Medium,
        )
        .on(&[E::Customer])
        .pii()
        .effects(&[S::Event])
        .requires(&["merchant.customer"]),
    ),
    (
        "merchant.order",
        F::read(Audience::Merchant, &["orders.read", "customers.pii"])
            .on(&[E::Order, E::Customer, E::Payment])
            .pii()
            .money(),
    ),
    (
        "merchant.order.transition",
        F::write(
            Audience::Merchant,
            &["orders.write", "customers.pii"],
            Reversibility::Irreversible,
            Risk::High,
        )
        .on(&[E::Order, E::Payment])
        .pii()
        .money()
        .stock()
        .effects(&[S::Email, S::Event])
        .requires(&["merchant.order"]),
    ),
    (
        "merchant.order.note",
        F::write(
            Audience::Merchant,
            &["orders.write", "customers.pii"],
            Reversibility::Irreversible,
            Risk::Medium,
        )
        .on(&[E::Order])
        .pii()
        .requires(&["merchant.order"]),
    ),
    (
        "merchant.delivery.split",
        F::write(
            Audience::Merchant,
            &["orders.write", "customers.pii"],
            Reversibility::Irreversible,
            Risk::Medium,
        )
        .on(&[E::Order])
        .pii()
        .stock()
        .effects(&[S::Event])
        .requires(&["merchant.order"]),
    ),
    (
        "merchant.receipts",
        F::read(Audience::Merchant, &["customers.pii", "documents.read"])
            .on(&[E::Document, E::Order])
            .pii()
            .money(),
    ),
    (
        "merchant.receipt.create",
        F::write(
            Audience::Merchant,
            &["customers.pii", "documents.create"],
            Reversibility::Irreversible,
            Risk::Medium,
        )
        .on(&[E::Document, E::Order])
        .pii()
        .money()
        .effects(&[S::Event])
        .requires(&["merchant.order"]),
    ),
    (
        "developer.archive",
        F::write(
            Audience::Merchant,
            &["developer.manage"],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::App]),
    ),
    (
        "developer.builds",
        F::read(Audience::Merchant, &["developer.read"]).on(&[E::App]),
    ),
    (
        "developer.import",
        F::write(
            Audience::Merchant,
            &["developer.manage"],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::App]),
    ),
    (
        "developer.stage",
        F::write(
            Audience::Merchant,
            &["developer.manage"],
            Reversibility::Reversible,
            Risk::Medium,
        )
        .on(&[E::App])
        .requires(&["developer.import"])
        .when(&[C::Sandbox]),
    ),
    (
        "developer.task",
        F::read(Audience::Merchant, &["developer.manage"]).on(&[E::App]),
    ),
    (
        "catalog.search",
        F::read(Audience::Shopper, &[])
            .on(&[E::Product])
            .money()
            .stock(),
    ),
    (
        "catalog.detail",
        F::read(Audience::Shopper, &[])
            .on(&[E::Product, E::Media])
            .money()
            .stock(),
    ),
    (
        "checkout.options",
        F::read(Audience::Shopper, &[])
            .on(&[E::Settings, E::Cart])
            .money(),
    ),
    (
        "checkout.select",
        F::write(Audience::Shopper, &[], Reversibility::Reversible, Risk::Low)
            .on(&[E::Cart])
            .pii()
            .money()
            .requires(&["cart.create"]),
    ),
    (
        "knowledge.graph",
        F::read(Audience::Merchant, &["knowledge.read"]).on(&[E::Knowledge, E::Product]),
    ),
    (
        "knowledge.search",
        F::read(Audience::Merchant, &["knowledge.read"])
            .on(&[E::Knowledge, E::Product])
            .money()
            .stock(),
    ),
    (
        "knowledge.external",
        F::read(Audience::Merchant, &["knowledge.read"])
            .on(&[E::Knowledge])
            .pii()
            .effects(&[S::ExternalCall]),
    ),
    (
        "knowledge.product",
        F::read(Audience::Merchant, &["knowledge.read"]).on(&[E::Knowledge, E::Product]),
    ),
    (
        "knowledge.workspace",
        F::read(Audience::Merchant, &["knowledge.read"]).on(&[E::Knowledge]),
    ),
    (
        "knowledge.preview",
        F::read(Audience::Merchant, &["knowledge.read"]).on(&[E::Knowledge]),
    ),
    (
        "knowledge.source.detail",
        F::read(Audience::Merchant, &["knowledge.read"]).on(&[E::Knowledge]),
    ),
    (
        "knowledge.source.create",
        F::write(
            Audience::Merchant,
            &["knowledge.write", "knowledge.read"],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::Knowledge])
        .effects(&[S::BackgroundJob, S::Event]),
    ),
    (
        "knowledge.source.edit",
        F::write(
            Audience::Merchant,
            &["knowledge.write", "knowledge.read"],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::Knowledge])
        .effects(&[S::BackgroundJob, S::Event])
        .requires(&["knowledge.source.detail"]),
    ),
    (
        "knowledge.source.visibility",
        F::write(
            Audience::Merchant,
            &["knowledge.write", "knowledge.read"],
            Reversibility::Reversible,
            Risk::Medium,
        )
        .on(&[E::Knowledge])
        .effects(&[S::Publication, S::Event])
        .requires(&["knowledge.source.detail"]),
    ),
    (
        "knowledge.source.archive",
        F::write(
            Audience::Merchant,
            &["knowledge.write", "knowledge.read"],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::Knowledge])
        .effects(&[S::Event])
        .requires(&["knowledge.source.detail"]),
    ),
    (
        "cart.create",
        F::write(Audience::Shopper, &[], Reversibility::Reversible, Risk::Low).on(&[E::Cart]),
    ),
    (
        "cart.replace",
        F::write(Audience::Shopper, &[], Reversibility::Reversible, Risk::Low)
            .on(&[E::Cart])
            .money()
            .requires(&["cart.create"]),
    ),
    (
        "cart.quote",
        F::read(Audience::Shopper, &[])
            .on(&[E::Cart])
            .money()
            .requires(&["cart.create"]),
    ),
    (
        "cart.offer",
        F::read(Audience::Shopper, &[])
            .on(&[E::Cart])
            .money()
            .requires(&["cart.create"]),
    ),
    (
        "offer.verify",
        F::read(Audience::Shopper, &[])
            .on(&[E::Cart])
            .money()
            .requires(&["cart.offer"]),
    ),
    (
        "checkout.complete",
        F::write(
            Audience::Shopper,
            &[],
            Reversibility::Irreversible,
            Risk::High,
        )
        .on(&[E::Order, E::Cart, E::Payment, E::Customer])
        .pii()
        .money()
        .stock()
        .effects(&[S::Payment, S::Email, S::Event])
        .requires(&["cart.create", "checkout.select"]),
    ),
    (
        "merchant.plan",
        F::write(
            Audience::Merchant,
            &[
                "assistant.use",
                "catalog.read",
                "orders.read",
                "knowledge.read",
            ],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::Run, E::Product])
        .money()
        .effects(&[S::ModelCall]),
    ),
    (
        "merchant.apply",
        F::write(
            Audience::Merchant,
            &[
                "assistant.use",
                "catalog.read",
                "catalog.write",
                "orders.read",
                "knowledge.read",
            ],
            Reversibility::Compensatable {
                by: "merchant.history.restore",
            },
            Risk::High,
        )
        .on(&[E::Product, E::Order])
        .money()
        .stock()
        .effects(&[S::Publication, S::Event])
        .requires(&["merchant.plan"]),
    ),
    (
        "merchant.orders",
        F::read(Audience::Merchant, &["orders.read", "customers.pii"])
            .on(&[E::Order, E::Customer])
            .pii()
            .money(),
    ),
    (
        "merchant.media.caption",
        F::read(Audience::Merchant, &["assistant.use"])
            .on(&[E::Media])
            .effects(&[S::ModelCall]),
    ),
    (
        "merchant.run.create",
        F::write(
            Audience::Merchant,
            &[
                "assistant.use",
                "catalog.read",
                "orders.read",
                "knowledge.read",
            ],
            Reversibility::Compensatable {
                by: "merchant.run.cancel",
            },
            Risk::Low,
        )
        .on(&[E::Run])
        .effects(&[S::ModelCall, S::BackgroundJob, S::Event]),
    ),
    (
        "merchant.run.status",
        F::read(
            Audience::Merchant,
            &[
                "assistant.use",
                "catalog.read",
                "orders.read",
                "knowledge.read",
            ],
        )
        .on(&[E::Run])
        .requires(&["merchant.run.create"]),
    ),
    (
        "merchant.run.explain",
        F::read(
            Audience::Merchant,
            &[
                "assistant.use",
                "catalog.read",
                "orders.read",
                "knowledge.read",
            ],
        )
        .on(&[E::Run])
        .requires(&["merchant.run.create"]),
    ),
    (
        "merchant.run.cancel",
        F::write(
            Audience::Merchant,
            &[
                "assistant.use",
                "catalog.read",
                "orders.read",
                "knowledge.read",
            ],
            Reversibility::Irreversible,
            Risk::Medium,
        )
        .on(&[E::Run])
        .effects(&[S::Event])
        .requires(&["merchant.run.create"]),
    ),
    (
        "merchant.run.resume",
        F::write(
            Audience::Merchant,
            &[
                "assistant.use",
                "catalog.read",
                "orders.read",
                "knowledge.read",
            ],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::Run])
        .effects(&[S::ModelCall, S::BackgroundJob])
        .requires(&["merchant.run.create"]),
    ),
    (
        "merchant.scenario.create",
        F::write(
            Audience::Merchant,
            &[
                "assistant.use",
                "channels.read",
                "environments.manage",
                "catalog.read",
                "orders.read",
                "orders.write",
                "knowledge.read",
            ],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::Run, E::Cart, E::Order])
        .effects(&[S::ModelCall, S::BackgroundJob])
        .requires(&["merchant.scenario.readiness"])
        .when(&[C::Sandbox]),
    ),
    (
        "merchant.scenario.status",
        F::read(
            Audience::Merchant,
            &[
                "assistant.use",
                "channels.read",
                "environments.manage",
                "catalog.read",
                "orders.read",
                "orders.write",
                "knowledge.read",
            ],
        )
        .on(&[E::Run, E::Order])
        .requires(&["merchant.scenario.create"])
        .when(&[C::Sandbox]),
    ),
    (
        "merchant.scenario.list",
        F::read(
            Audience::Merchant,
            &[
                "assistant.use",
                "channels.read",
                "environments.manage",
                "catalog.read",
                "orders.read",
                "orders.write",
                "knowledge.read",
            ],
        )
        .on(&[E::Run])
        .when(&[C::Sandbox]),
    ),
    (
        "merchant.scenario.readiness",
        F::read(
            Audience::Merchant,
            &[
                "assistant.use",
                "channels.read",
                "environments.manage",
                "catalog.read",
                "orders.read",
                "orders.write",
                "knowledge.read",
            ],
        )
        .on(&[E::Run, E::Product])
        .stock()
        .when(&[C::Sandbox]),
    ),
    (
        "merchant.scenario.stock",
        F::write(
            Audience::Merchant,
            &[
                "assistant.use",
                "channels.read",
                "environments.manage",
                "catalog.read",
                "orders.read",
                "orders.write",
                "knowledge.read",
            ],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::Product])
        .stock()
        .requires(&["merchant.scenario.readiness"])
        .when(&[C::Sandbox]),
    ),
    (
        "merchant.scenario.attacks",
        F::read(
            Audience::Merchant,
            &[
                "assistant.use",
                "channels.read",
                "environments.manage",
                "catalog.read",
                "orders.read",
                "orders.write",
                "knowledge.read",
            ],
        )
        .on(&[E::Run])
        .requires(&["merchant.scenario.probe"])
        .when(&[C::Sandbox]),
    ),
    (
        "merchant.scenario.probe",
        F::write(
            Audience::Merchant,
            &[
                "assistant.use",
                "channels.read",
                "environments.manage",
                "catalog.read",
                "orders.read",
                "orders.write",
                "knowledge.read",
            ],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::Run, E::Cart, E::Order])
        .money()
        .effects(&[S::BackgroundJob])
        .requires(&["merchant.scenario.readiness"])
        .when(&[C::Sandbox]),
    ),
    (
        "merchant.apps.search",
        F::read(Audience::Merchant, &["apps.read"]).on(&[E::App]),
    ),
    (
        "merchant.apps.read",
        F::read(Audience::Merchant, &["apps.read"])
            .on(&[E::App])
            .effects(&[S::ExternalCall])
            .requires(&["merchant.apps.search"]),
    ),
    (
        "merchant.agents.status",
        F::read(Audience::Merchant, &["ai.manage"])
            .on(&[E::Run, E::Settings])
            .when(&[C::PersonalSession]),
    ),
    (
        "merchant.agents.control",
        F::write(
            Audience::Merchant,
            &["ai.manage"],
            Reversibility::Reversible,
            Risk::Medium,
        )
        .on(&[E::Run, E::Settings])
        .effects(&[S::Event])
        .requires(&["merchant.agents.status"])
        .when(&[C::PersonalSession]),
    ),
    (
        "merchant.grid.plan",
        F::read(Audience::Merchant, &["assistant.use", "catalog.read"])
            .on(&[E::Product, E::Order, E::Customer])
            .effects(&[S::ModelCall]),
    ),
    (
        "merchant.customer.lookup",
        F::read(Audience::Merchant, &["customers.read", "customers.pii"])
            .on(&[E::Customer])
            .pii(),
    ),
    (
        "team.roles",
        F::read(Audience::Merchant, &["team.manage"]).on(&[E::Role]),
    ),
    (
        "team.role.save",
        F::write(
            Audience::Merchant,
            &["team.manage"],
            Reversibility::Reversible,
            Risk::High,
        )
        .on(&[E::Role]),
    ),
    (
        "team.role.dependencies",
        F::read(Audience::Merchant, &["team.manage"]).on(&[E::Role]),
    ),
    (
        "team.role.delete",
        F::write(
            Audience::Merchant,
            &["team.manage"],
            Reversibility::Irreversible,
            Risk::High,
        )
        .on(&[E::Role])
        .requires(&["team.role.dependencies"]),
    ),
    (
        "knowledge.behavior",
        F::read(Audience::Merchant, &["knowledge.read"]).on(&[E::Knowledge, E::Report]),
    ),
    (
        "knowledge.behavior.journey",
        F::read(Audience::Merchant, &["customers.pii"])
            .on(&[E::Knowledge, E::Customer])
            .pii(),
    ),
    (
        "knowledge.experiments",
        F::read(Audience::Merchant, &["knowledge.read"]).on(&[E::Experiment]),
    ),
    (
        "knowledge.experiment.create",
        F::write(
            Audience::Merchant,
            &["knowledge.read"],
            Reversibility::Irreversible,
            Risk::Medium,
        )
        .on(&[E::Experiment])
        .money()
        .effects(&[S::Event]),
    ),
    (
        "knowledge.experiment.start",
        F::write(
            Audience::Merchant,
            &["knowledge.read"],
            Reversibility::Compensatable {
                by: "knowledge.experiment.stop",
            },
            Risk::High,
        )
        .on(&[E::Experiment])
        .money()
        .effects(&[S::Publication, S::Event])
        .requires(&["knowledge.experiment.create"]),
    ),
    (
        "knowledge.experiment.stop",
        F::write(
            Audience::Merchant,
            &["knowledge.read"],
            Reversibility::Irreversible,
            Risk::Medium,
        )
        .on(&[E::Experiment])
        .effects(&[S::Event])
        .requires(&["knowledge.experiment.start"]),
    ),
    (
        "knowledge.experiment.finish",
        F::write(
            Audience::Merchant,
            &["knowledge.read"],
            Reversibility::Irreversible,
            Risk::Medium,
        )
        .on(&[E::Experiment])
        .effects(&[S::Event])
        .requires(&["knowledge.experiment.start"]),
    ),
    (
        "knowledge.experiment.suggest",
        F::read(Audience::Merchant, &["knowledge.read"])
            .on(&[E::Experiment, E::Payment])
            .money(),
    ),
    (
        "knowledge.experiment.report",
        F::read(Audience::Merchant, &["knowledge.read"])
            .on(&[E::Experiment, E::Payment])
            .money()
            .requires(&["knowledge.experiment.create"]),
    ),
    (
        "catalog.facts",
        F::read(Audience::Shopper, &[])
            .on(&[E::Product])
            .money()
            .stock(),
    ),
    (
        "knowledge.autonomy.apply",
        F::write(
            Audience::Merchant,
            &["catalog.write", "settings.write", "knowledge.read"],
            Reversibility::Compensatable {
                by: "knowledge.autonomy.revert",
            },
            Risk::High,
        )
        .on(&[E::Product])
        .money()
        .effects(&[S::Publication, S::Event])
        .requires(&["knowledge.autonomy.status"]),
    ),
    (
        "knowledge.autonomy.revert",
        F::write(
            Audience::Merchant,
            &["catalog.write", "settings.write", "knowledge.read"],
            Reversibility::Reversible,
            Risk::Medium,
        )
        .on(&[E::Product])
        .money()
        .effects(&[S::Publication, S::Event])
        .requires(&["knowledge.autonomy.apply"]),
    ),
    (
        "knowledge.autonomy.status",
        F::read(Audience::Merchant, &["knowledge.read"])
            .on(&[E::Product, E::Settings])
            .money(),
    ),
    (
        "knowledge.claims",
        F::read(Audience::Merchant, &["knowledge.read"]).on(&[E::Knowledge, E::Product]),
    ),
    (
        "knowledge.claim.propose",
        F::write(
            Audience::Merchant,
            &["knowledge.write", "knowledge.read"],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::Knowledge]),
    ),
    (
        "knowledge.claim.review",
        F::write(
            Audience::Merchant,
            &["knowledge.write", "knowledge.read"],
            Reversibility::Reversible,
            Risk::Medium,
        )
        .on(&[E::Knowledge])
        .effects(&[S::Event])
        .requires(&["knowledge.claim.propose"]),
    ),
    (
        "knowledge.extract",
        F::write(
            Audience::Merchant,
            &["knowledge.write", "knowledge.read"],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::Knowledge])
        .effects(&[S::ModelCall])
        .requires(&["knowledge.source.detail"]),
    ),
    (
        "knowledge.compile",
        F::write(
            Audience::Merchant,
            &["knowledge.read"],
            Reversibility::Reversible,
            Risk::Medium,
        )
        .on(&[E::Knowledge, E::Product])
        .effects(&[S::Publication])
        .requires(&["knowledge.claim.review"]),
    ),
    (
        "checkout.payment.methods",
        F::read(Audience::Shopper, &[]).on(&[E::Payment]).pii(),
    ),
    (
        "checkout.payment.method.revoke",
        F::write(
            Audience::Shopper,
            &[],
            Reversibility::Irreversible,
            Risk::High,
        )
        .on(&[E::Payment])
        .pii()
        .effects(&[S::ExternalCall, S::BackgroundJob, S::Event])
        .requires(&["checkout.payment.methods"]),
    ),
    (
        "checkout.payment.instructions",
        F::read(Audience::Shopper, &[])
            .on(&[E::Payment, E::Order])
            .pii()
            .money(),
    ),
    (
        "merchant.payment.instructions",
        F::read(
            Audience::Merchant,
            &["orders.read", "customers.pii", "payments.read"],
        )
        .on(&[E::Payment, E::Order])
        .pii()
        .money(),
    ),
    (
        "merchant.payment",
        F::write(
            Audience::Merchant,
            &["payments.manage"],
            Reversibility::Irreversible,
            Risk::High,
        )
        .on(&[E::Payment, E::Order])
        .money()
        .effects(&[S::Payment, S::ExternalCall, S::Event])
        .requires(&["merchant.payment.details"]),
    ),
    (
        "merchant.payment.details",
        F::read(Audience::Merchant, &["payments.read"])
            .on(&[E::Payment, E::Order])
            .money(),
    ),
    (
        "merchant.payment.providers",
        F::read(Audience::Merchant, &["payments.read"]).on(&[E::Payment, E::Settings]),
    ),
    (
        "merchant.payment.onboarding",
        F::write(
            Audience::Merchant,
            &["payments.manage"],
            Reversibility::Irreversible,
            Risk::High,
        )
        .on(&[E::Payment, E::Settings])
        .effects(&[S::ExternalCall, S::Event])
        .requires(&["merchant.payment.providers"]),
    ),
    (
        "merchant.app.history",
        F::read(Audience::Merchant, &["apps.read", "catalog.read"]).on(&[E::App, E::History]),
    ),
    (
        "merchant.app.restore",
        F::write(
            Audience::Merchant,
            &["catalog.read", "apps.manage"],
            Reversibility::Reversible,
            Risk::Medium,
        )
        .on(&[E::App])
        .effects(&[S::Event])
        .requires(&["merchant.app.history"]),
    ),
    (
        "merchant.site.task",
        F::read(
            Audience::Merchant,
            &["assistant.use", "channels.read", "channels.manage"],
        )
        .on(&[E::Site]),
    ),
    (
        "merchant.site.propose",
        F::write(
            Audience::Merchant,
            &["assistant.use", "channels.read", "channels.manage"],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::Site])
        .requires(&["merchant.site.task"]),
    ),
    (
        "merchant.site.plan",
        F::write(
            Audience::Merchant,
            &["assistant.use", "channels.read", "channels.manage"],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::Site])
        .effects(&[S::ModelCall]),
    ),
    (
        "merchant.site.apply",
        F::write(
            Audience::Merchant,
            &["assistant.use", "channels.read", "channels.manage"],
            Reversibility::Reversible,
            Risk::Medium,
        )
        .on(&[E::Site])
        .requires(&["merchant.site.plan"])
        .when(&[C::Sandbox]),
    ),
    (
        "merchant.reports.read",
        F::read(Audience::Merchant, &["overview.read"])
            .on(&[E::Report, E::Order])
            .money(),
    ),
    (
        "studio.workspace.work.read",
        F::read(Audience::Merchant, &[])
            .on(&[E::Workspace, E::Order, E::Product])
            .when(&[C::PersonalSession]),
    ),
    (
        "studio.workspace.read",
        F::read(Audience::Merchant, &[])
            .on(&[E::Workspace])
            .when(&[C::PersonalSession]),
    ),
    (
        "studio.workspace.save",
        F::write(
            Audience::Merchant,
            &[],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::Workspace])
        .requires(&["studio.workspace.read"])
        .when(&[C::PersonalSession]),
    ),
    (
        "merchant.import.inspect",
        F::read(Audience::Merchant, &["apps.manage"]).on(&[
            E::Import,
            E::Product,
            E::Customer,
            E::Order,
        ]),
    ),
    (
        "merchant.import.record",
        F::write(
            Audience::Merchant,
            &["apps.manage"],
            Reversibility::Irreversible,
            Risk::Medium,
        )
        .on(&[E::Import, E::Product, E::Customer, E::Order])
        .pii()
        .money()
        .stock()
        .effects(&[S::Event])
        .requires(&["merchant.import.inspect"]),
    ),
    (
        "merchant.customer.creation",
        F::read(Audience::Merchant, &["customers.pii", "customers.write"])
            .on(&[E::Customer, E::Settings]),
    ),
    (
        "merchant.customer.create",
        F::write(
            Audience::Merchant,
            &["customers.pii", "customers.write"],
            Reversibility::Irreversible,
            Risk::Medium,
        )
        .on(&[E::Customer])
        .pii()
        .effects(&[S::Event])
        .requires(&["merchant.customer.creation"]),
    ),
    (
        "merchant.order.context",
        F::read(
            Audience::Merchant,
            &[
                "catalog.read",
                "orders.write",
                "customers.read",
                "customers.pii",
            ],
        )
        .on(&[E::Order, E::Customer, E::Settings])
        .pii(),
    ),
    (
        "merchant.order.review",
        F::write(
            Audience::Merchant,
            &[
                "catalog.read",
                "orders.write",
                "customers.read",
                "customers.pii",
            ],
            Reversibility::Reversible,
            Risk::Medium,
        )
        .on(&[E::Cart, E::Order, E::Customer])
        .pii()
        .money()
        .requires(&["merchant.order.context"]),
    ),
    (
        "merchant.order.complete",
        F::write(
            Audience::Merchant,
            &[
                "catalog.read",
                "orders.write",
                "customers.read",
                "customers.pii",
            ],
            Reversibility::Irreversible,
            Risk::High,
        )
        .on(&[E::Order, E::Customer, E::Cart])
        .pii()
        .money()
        .stock()
        .effects(&[S::Event])
        .requires(&["merchant.order.review"]),
    ),
    (
        "merchant.catalog.context",
        F::read(Audience::Merchant, &["catalog.read"]).on(&[E::Product, E::Settings]),
    ),
    (
        "merchant.catalog.prepare",
        F::read(Audience::Merchant, &["catalog.read", "catalog.write"])
            .on(&[E::Product, E::Import])
            .effects(&[S::ModelCall, S::ExternalCall]),
    ),
    (
        "merchant.catalog.save",
        F::write(
            Audience::Merchant,
            &["catalog.read", "catalog.write"],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::Product])
        .money()
        .stock()
        .requires(&["merchant.catalog.prepare"])
        .when(&[C::Sandbox]),
    ),
    (
        "pos.customers",
        F::read(Audience::Merchant, &["pos.use"])
            .on(&[E::Customer])
            .pii(),
    ),
    (
        "pos.customer",
        F::write(
            Audience::Merchant,
            &["pos.use"],
            Reversibility::Reversible,
            Risk::Medium,
        )
        .on(&[E::Cart, E::Customer])
        .pii()
        .requires(&["pos.create"]),
    ),
    (
        "pos.discount",
        F::write(
            Audience::Merchant,
            &["pos.use"],
            Reversibility::Reversible,
            Risk::Medium,
        )
        .on(&[E::Cart])
        .money()
        .requires(&["pos.create"]),
    ),
    (
        "pos.receipt",
        F::read(Audience::Merchant, &["pos.use"])
            .on(&[E::Document, E::Order])
            .money(),
    ),
    (
        "pos.channels",
        F::read(Audience::Merchant, &["pos.use"]).on(&[E::Settings]),
    ),
    (
        "pos.payment",
        F::read(Audience::Merchant, &["pos.use"])
            .on(&[E::Payment, E::Order])
            .money(),
    ),
    (
        "pos.reconcile",
        F::write(
            Audience::Merchant,
            &["pos.use"],
            Reversibility::Reversible,
            Risk::Medium,
        )
        .on(&[E::Payment, E::Order])
        .money()
        .effects(&[S::ExternalCall, S::BackgroundJob])
        .requires(&["pos.payment"]),
    ),
    (
        "pos.catalog",
        F::read(Audience::Merchant, &["pos.use"])
            .on(&[E::Product])
            .money()
            .stock(),
    ),
    (
        "pos.context",
        F::read(Audience::Merchant, &["pos.use"]).on(&[E::Settings]),
    ),
    (
        "pos.detail",
        F::read(Audience::Merchant, &["pos.use"])
            .on(&[E::Product])
            .money()
            .stock(),
    ),
    (
        "pos.currency",
        F::write(
            Audience::Merchant,
            &["pos.use"],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::Cart, E::Currency])
        .money()
        .requires(&["pos.create"]),
    ),
    (
        "pos.coupons",
        F::write(
            Audience::Merchant,
            &["pos.use"],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::Cart])
        .money()
        .requires(&["pos.create"]),
    ),
    (
        "pos.create",
        F::write(
            Audience::Merchant,
            &["pos.use"],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::Cart]),
    ),
    (
        "pos.cart",
        F::read(Audience::Merchant, &["pos.use"])
            .on(&[E::Cart])
            .money()
            .requires(&["pos.create"]),
    ),
    (
        "pos.replace",
        F::write(
            Audience::Merchant,
            &["pos.use"],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::Cart])
        .money()
        .requires(&["pos.create"]),
    ),
    (
        "pos.select",
        F::write(
            Audience::Merchant,
            &["pos.use"],
            Reversibility::Reversible,
            Risk::Medium,
        )
        .on(&[E::Cart, E::Customer])
        .pii()
        .requires(&["pos.create"]),
    ),
    (
        "pos.accept",
        F::write(
            Audience::Merchant,
            &["pos.use"],
            Reversibility::Irreversible,
            Risk::Medium,
        )
        .on(&[E::Legal, E::Cart])
        .requires(&["pos.create"]),
    ),
    (
        "pos.complete",
        F::write(
            Audience::Merchant,
            &["pos.use"],
            Reversibility::Irreversible,
            Risk::High,
        )
        .on(&[E::Order, E::Payment, E::Cart])
        .money()
        .stock()
        .effects(&[S::Payment, S::Event])
        .requires(&["pos.select"]),
    ),
    (
        "pos.held",
        F::read(Audience::Merchant, &["pos.use"])
            .on(&[E::Cart])
            .money(),
    ),
    (
        "pos.sales",
        F::read(Audience::Merchant, &["pos.use"])
            .on(&[E::Order, E::Payment])
            .money(),
    ),
    (
        "b2b.company.create",
        F::write(
            Audience::Merchant,
            &["b2b.manage"],
            Reversibility::Reversible,
            Risk::Medium,
        )
        .on(&[E::Company, E::Customer])
        .pii()
        .effects(&[S::Event]),
    ),
    (
        "b2b.settings.save",
        F::write(
            Audience::Merchant,
            &["settings.write"],
            Reversibility::Reversible,
            Risk::Medium,
        )
        .on(&[E::Settings]),
    ),
    (
        "b2b.configuration",
        F::read(Audience::Merchant, &["b2b.read"]).on(&[E::Settings, E::Company]),
    ),
    (
        "b2b.requisition.import",
        F::write(
            Audience::Merchant,
            &["b2b.manage"],
            Reversibility::Reversible,
            Risk::Low,
        )
        .on(&[E::List, E::Product]),
    ),
    (
        "b2b.unit.orders",
        F::read(Audience::Shopper, &[])
            .on(&[E::Order, E::Company])
            .pii()
            .money(),
    ),
    (
        "b2b.catalog.save",
        F::write(
            Audience::Merchant,
            &["b2b.manage"],
            Reversibility::Reversible,
            Risk::Medium,
        )
        .on(&[E::Company, E::Product])
        .effects(&[S::Event]),
    ),
    (
        "b2b.quotes",
        F::read(Audience::Shopper, &[]).on(&[E::Quote]),
    ),
    (
        "b2b.quote.get",
        F::read(Audience::Shopper, &[])
            .on(&[E::Quote, E::Cart])
            .money(),
    ),
    (
        "b2b.quote.current",
        F::read(Audience::Shopper, &[])
            .on(&[E::Quote, E::Cart])
            .money(),
    ),
    (
        "b2b.quotes.merchant",
        F::read(Audience::Merchant, &["b2b.manage"])
            .on(&[E::Quote, E::Company])
            .pii()
            .money(),
    ),
    (
        "b2b.quote.request",
        F::write(
            Audience::Shopper,
            &[],
            Reversibility::Compensatable {
                by: "b2b.quote.reject",
            },
            Risk::Low,
        )
        .on(&[E::Quote, E::Cart])
        .money()
        .effects(&[S::Event]),
    ),
    (
        "b2b.quote.offer",
        F::write(
            Audience::Merchant,
            &["b2b.manage"],
            Reversibility::Irreversible,
            Risk::High,
        )
        .on(&[E::Quote])
        .money()
        .effects(&[S::Event])
        .requires(&["b2b.quote.request"]),
    ),
    (
        "b2b.quote.transition",
        F::write(
            Audience::Merchant,
            &["b2b.manage"],
            Reversibility::Irreversible,
            Risk::Medium,
        )
        .on(&[E::Quote])
        .effects(&[S::Event])
        .requires(&["b2b.quote.accept"]),
    ),
    (
        "b2b.quote.message",
        F::write(
            Audience::Shopper,
            &[],
            Reversibility::Irreversible,
            Risk::Medium,
        )
        .on(&[E::Quote])
        .effects(&[S::Event])
        .requires(&["b2b.quote.request"]),
    ),
    (
        "b2b.quote.reject",
        F::write(
            Audience::Shopper,
            &[],
            Reversibility::Irreversible,
            Risk::Medium,
        )
        .on(&[E::Quote])
        .effects(&[S::Event])
        .requires(&["b2b.quote.request"]),
    ),
    (
        "b2b.quote.accept",
        F::write(
            Audience::Shopper,
            &[],
            Reversibility::Compensatable {
                by: "b2b.quote.release",
            },
            Risk::Medium,
        )
        .on(&[E::Quote, E::Cart])
        .money()
        .effects(&[S::Event])
        .requires(&["b2b.quote.offer"]),
    ),
    (
        "b2b.quote.release",
        F::write(
            Audience::Shopper,
            &[],
            Reversibility::Compensatable {
                by: "b2b.quote.accept",
            },
            Risk::Low,
        )
        .on(&[E::Quote, E::Cart])
        .money()
        .requires(&["b2b.quote.accept"]),
    ),
    ("b2b.lists", F::read(Audience::Shopper, &[]).on(&[E::List])),
    (
        "b2b.list.save",
        F::write(Audience::Shopper, &[], Reversibility::Reversible, Risk::Low)
            .on(&[E::List])
            .effects(&[S::Event]),
    ),
    (
        "b2b.list.delete",
        F::write(
            Audience::Shopper,
            &[],
            Reversibility::Irreversible,
            Risk::Medium,
        )
        .on(&[E::List])
        .effects(&[S::Event])
        .requires(&["b2b.lists"]),
    ),
    (
        "b2b.list.apply",
        F::write(Audience::Shopper, &[], Reversibility::Reversible, Risk::Low)
            .on(&[E::Cart, E::List])
            .money()
            .requires(&["b2b.lists"]),
    ),
    (
        "b2b.quick-order",
        F::write(Audience::Shopper, &[], Reversibility::Reversible, Risk::Low)
            .on(&[E::Cart, E::Product])
            .money(),
    ),
    (
        "b2b.companies",
        F::read(Audience::Merchant, &["b2b.read"])
            .on(&[E::Company])
            .pii(),
    ),
    (
        "b2b.company",
        F::read(Audience::Merchant, &["b2b.read"])
            .on(&[E::Company, E::Customer])
            .pii(),
    ),
    (
        "b2b.company.save",
        F::write(
            Audience::Merchant,
            &["b2b.manage"],
            Reversibility::Reversible,
            Risk::Medium,
        )
        .on(&[E::Company])
        .pii()
        .effects(&[S::Event])
        .requires(&["b2b.company"]),
    ),
    (
        "b2b.unit.save",
        F::write(
            Audience::Merchant,
            &["b2b.manage"],
            Reversibility::Reversible,
            Risk::Medium,
        )
        .on(&[E::Company])
        .effects(&[S::Event])
        .requires(&["b2b.company"]),
    ),
    (
        "b2b.member.save",
        F::write(
            Audience::Merchant,
            &["b2b.manage"],
            Reversibility::Reversible,
            Risk::Medium,
        )
        .on(&[E::Company, E::Customer])
        .pii()
        .money()
        .effects(&[S::Event])
        .requires(&["b2b.company"]),
    ),
    (
        "b2b.contexts",
        F::read(Audience::Shopper, &[]).on(&[E::Company]),
    ),
    (
        "b2b.select",
        F::write(Audience::Shopper, &[], Reversibility::Reversible, Risk::Low)
            .on(&[E::Cart, E::Company])
            .requires(&["b2b.contexts"]),
    ),
    (
        "b2b.approval.request",
        F::write(Audience::Shopper, &[], Reversibility::Reversible, Risk::Low)
            .on(&[E::Quote, E::Company])
            .money()
            .effects(&[S::Event])
            .requires(&["b2b.quote.get"]),
    ),
    (
        "b2b.approvals",
        F::read(Audience::Shopper, &[])
            .on(&[E::Quote, E::Company])
            .pii()
            .money(),
    ),
    (
        "b2b.approval.decide",
        F::write(
            Audience::Shopper,
            &[],
            Reversibility::Irreversible,
            Risk::High,
        )
        .on(&[E::Quote, E::Order, E::Company])
        .money()
        .effects(&[S::Event])
        .requires(&["b2b.approval.request"]),
    ),
    (
        "automation.compare_history",
        F::read(
            Audience::Merchant,
            &[
                "automation.read",
                "b2b.read",
                "catalog.read",
                "orders.read",
                "customers.read",
                "customers.pii",
                "documents.read",
                "payments.read",
            ],
        )
        .on(&[E::Automation, E::Order])
        .pii()
        .money(),
    ),
    (
        "automation.validate_flow",
        F::read(Audience::Merchant, &["automation.manage"]).on(&[E::Automation]),
    ),
    (
        "merchant.operations.list",
        F::read(Audience::Merchant, &[]).on(&[E::Operation]),
    ),
    (
        "merchant.operations.check",
        F::read(Audience::Merchant, &[])
            .on(&[E::Operation])
            .requires(&["merchant.operations.list"]),
    ),
];
