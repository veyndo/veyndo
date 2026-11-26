-- schema
ALTER SEQUENCE public.app_action_log_id_seq OWNED BY public.app_action_log.id;

ALTER SEQUENCE public.chat_messages_id_seq OWNED BY public.chat_messages.id;

ALTER SEQUENCE public.entity_history_id_seq OWNED BY public.entity_history.id;

ALTER SEQUENCE public.order_activity_id_seq OWNED BY public.order_activity.id;

ALTER SEQUENCE public.outbox_dispatch_sequence OWNED BY public.outbox_tenant_dispatch.served_order;

ALTER SEQUENCE public.outbox_id_seq OWNED BY public.outbox.id;

ALTER SEQUENCE public.platform_audit_id_seq OWNED BY public.platform_audit.id;

ALTER TABLE ONLY public.ai_price_budget
    ADD CONSTRAINT ai_price_budget_pkey PRIMARY KEY (tenant, day, product_id);

ALTER TABLE ONLY public.ai_price_budget
    ADD CONSTRAINT ai_price_budget_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.ai_price_budget
    ADD CONSTRAINT ai_price_budget_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.ai_price_budget
    ADD CONSTRAINT ai_price_budget_tenant_task_id_fkey FOREIGN KEY (tenant, task_id) REFERENCES public.tasks(tenant, id);

ALTER TABLE ONLY public.ai_price_budget FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.app_action_log
    ADD CONSTRAINT app_action_log_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.app_action_log
    ADD CONSTRAINT app_action_log_tenant_app_fkey FOREIGN KEY (tenant, app) REFERENCES public.app_packages(tenant, id);

ALTER TABLE ONLY public.app_action_log ALTER COLUMN id SET DEFAULT nextval('public.app_action_log_id_seq'::regclass);

ALTER TABLE ONLY public.app_action_log FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.app_deliveries
    ADD CONSTRAINT app_deliveries_event_id_fkey FOREIGN KEY (event_id) REFERENCES public.outbox(id);

ALTER TABLE ONLY public.app_deliveries
    ADD CONSTRAINT app_deliveries_event_scope FOREIGN KEY (tenant, event_id) REFERENCES public.outbox(tenant, id);

ALTER TABLE ONLY public.app_deliveries
    ADD CONSTRAINT app_deliveries_pkey PRIMARY KEY (tenant, app, event_id);

ALTER TABLE ONLY public.app_deliveries
    ADD CONSTRAINT app_deliveries_tenant_app_fkey FOREIGN KEY (tenant, app) REFERENCES public.app_packages(tenant, id);

ALTER TABLE ONLY public.app_deliveries FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.app_evidence
    ADD CONSTRAINT app_evidence_pkey PRIMARY KEY (tenant, app, source_id);

ALTER TABLE ONLY public.app_evidence
    ADD CONSTRAINT app_evidence_tenant_app_fkey FOREIGN KEY (tenant, app) REFERENCES public.app_packages(tenant, id);

ALTER TABLE ONLY public.app_evidence
    ADD CONSTRAINT app_evidence_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.app_evidence FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.app_export_cursors
    ADD CONSTRAINT app_export_cursors_pkey PRIMARY KEY (tenant, app);

ALTER TABLE ONLY public.app_export_cursors FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.app_jobs
    ADD CONSTRAINT app_jobs_pkey PRIMARY KEY (tenant, app, id);

ALTER TABLE ONLY public.app_jobs
    ADD CONSTRAINT app_jobs_tenant_app_fkey FOREIGN KEY (tenant, app) REFERENCES public.app_packages(tenant, id);

ALTER TABLE ONLY public.app_jobs
    ADD CONSTRAINT app_jobs_tenant_app_request_key_key UNIQUE (tenant, app, request_key);

ALTER TABLE ONLY public.app_jobs FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.app_models
    ADD CONSTRAINT app_models_pkey PRIMARY KEY (tenant, app, entity);

ALTER TABLE ONLY public.app_models
    ADD CONSTRAINT app_models_tenant_app_fkey FOREIGN KEY (tenant, app) REFERENCES public.app_packages(tenant, id) DEFERRABLE INITIALLY DEFERRED;

ALTER TABLE ONLY public.app_models FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.app_packages
    ADD CONSTRAINT app_packages_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.app_packages
    ADD CONSTRAINT app_packages_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.app_packages FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.app_record_indexes
    ADD CONSTRAINT app_record_indexes_pkey PRIMARY KEY (tenant, app, entity, field, record_id);

ALTER TABLE ONLY public.app_record_indexes
    ADD CONSTRAINT app_record_indexes_tenant_app_entity_record_id_fkey FOREIGN KEY (tenant, app, entity, record_id) REFERENCES public.app_records(tenant, app, entity, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.app_record_indexes FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.app_record_references
    ADD CONSTRAINT app_record_references_pkey PRIMARY KEY (tenant, app, entity, record_id, field, "position");

ALTER TABLE ONLY public.app_record_references
    ADD CONSTRAINT app_record_references_tenant_app_entity_record_id_fkey FOREIGN KEY (tenant, app, entity, record_id) REFERENCES public.app_records(tenant, app, entity, id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED;

ALTER TABLE ONLY public.app_record_references
    ADD CONSTRAINT app_record_references_tenant_app_target_entity_target_id_fkey FOREIGN KEY (tenant, app, target_entity, target_id) REFERENCES public.app_records(tenant, app, entity, id) DEFERRABLE INITIALLY DEFERRED;

ALTER TABLE ONLY public.app_record_references
    ADD CONSTRAINT app_record_references_tenant_asset_id_fkey FOREIGN KEY (tenant, asset_id) REFERENCES public.product_assets(tenant, id);

ALTER TABLE ONLY public.app_record_references
    ADD CONSTRAINT app_record_references_tenant_channel_id_fkey FOREIGN KEY (tenant, channel_id) REFERENCES public.sales_channels(tenant, id);

ALTER TABLE ONLY public.app_record_references
    ADD CONSTRAINT app_record_references_tenant_company_id_fkey FOREIGN KEY (tenant, company_id) REFERENCES public.b2b_companies(tenant, id);

ALTER TABLE ONLY public.app_record_references
    ADD CONSTRAINT app_record_references_tenant_customer_id_fkey FOREIGN KEY (tenant, customer_id) REFERENCES public.customers(tenant, id);

ALTER TABLE ONLY public.app_record_references
    ADD CONSTRAINT app_record_references_tenant_order_id_fkey FOREIGN KEY (tenant, order_id) REFERENCES public.orders(tenant, id);

ALTER TABLE ONLY public.app_record_references
    ADD CONSTRAINT app_record_references_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.app_record_references
    ADD CONSTRAINT app_record_references_tenant_unit_id_fkey FOREIGN KEY (tenant, unit_id) REFERENCES public.b2b_units(tenant, id);

ALTER TABLE ONLY public.app_record_references FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.app_record_versions
    ADD CONSTRAINT app_record_versions_pkey PRIMARY KEY (tenant, app, entity, record_id, revision);

ALTER TABLE ONLY public.app_record_versions
    ADD CONSTRAINT app_record_versions_tenant_app_fkey FOREIGN KEY (tenant, app) REFERENCES public.app_packages(tenant, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.app_record_versions FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.app_records
    ADD CONSTRAINT app_records_pkey PRIMARY KEY (tenant, app, entity, id);

ALTER TABLE ONLY public.app_records
    ADD CONSTRAINT app_records_tenant_app_entity_fkey FOREIGN KEY (tenant, app, entity) REFERENCES public.app_models(tenant, app, entity);

ALTER TABLE ONLY public.app_records FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.app_schedule_runs
    ADD CONSTRAINT app_schedule_runs_event_id_fkey FOREIGN KEY (event_id) REFERENCES public.outbox(id);

ALTER TABLE ONLY public.app_schedule_runs
    ADD CONSTRAINT app_schedule_runs_pkey PRIMARY KEY (tenant, app, schedule, due);

ALTER TABLE ONLY public.app_schedule_runs
    ADD CONSTRAINT schedule_runs_event_scope FOREIGN KEY (tenant, event_id) REFERENCES public.outbox(tenant, id);

ALTER TABLE ONLY public.app_schedule_runs FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.app_schedules
    ADD CONSTRAINT app_schedules_pkey PRIMARY KEY (tenant, app, id);

ALTER TABLE ONLY public.app_schedules
    ADD CONSTRAINT app_schedules_tenant_app_fkey FOREIGN KEY (tenant, app) REFERENCES public.app_packages(tenant, id);

ALTER TABLE ONLY public.app_schedules FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.app_schema_history
    ADD CONSTRAINT app_schema_history_pkey PRIMARY KEY (tenant, app, to_version);

ALTER TABLE ONLY public.app_schema_history
    ADD CONSTRAINT app_schema_history_tenant_app_fkey FOREIGN KEY (tenant, app) REFERENCES public.app_packages(tenant, id);

ALTER TABLE ONLY public.app_schema_history FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.app_service_secrets
    ADD CONSTRAINT app_service_secrets_pkey PRIMARY KEY (tenant, app, kind);

ALTER TABLE ONLY public.app_service_secrets
    ADD CONSTRAINT app_service_secrets_tenant_app_fkey FOREIGN KEY (tenant, app) REFERENCES public.app_packages(tenant, id);

ALTER TABLE ONLY public.app_service_secrets FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.app_storage_usage
    ADD CONSTRAINT app_storage_usage_pkey PRIMARY KEY (tenant, app);

ALTER TABLE ONLY public.app_storage_usage
    ADD CONSTRAINT app_storage_usage_tenant_app_fkey FOREIGN KEY (tenant, app) REFERENCES public.app_packages(tenant, id);

ALTER TABLE ONLY public.app_storage_usage FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.app_surface_grants
    ADD CONSTRAINT app_surface_grants_pkey PRIMARY KEY (tenant, token_hash);

ALTER TABLE ONLY public.app_surface_grants
    ADD CONSTRAINT app_surface_grants_tenant_app_fkey FOREIGN KEY (tenant, app) REFERENCES public.app_packages(tenant, id);

ALTER TABLE ONLY public.app_surface_grants FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.app_versions
    ADD CONSTRAINT app_versions_pkey PRIMARY KEY (tenant, app, version);

ALTER TABLE ONLY public.app_versions FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.app_webhook_receipts
    ADD CONSTRAINT app_webhook_receipts_event_id_fkey FOREIGN KEY (event_id) REFERENCES public.outbox(id);

ALTER TABLE ONLY public.app_webhook_receipts
    ADD CONSTRAINT app_webhook_receipts_pkey PRIMARY KEY (tenant, app, webhook, request_key);

ALTER TABLE ONLY public.app_webhook_receipts
    ADD CONSTRAINT webhook_receipts_event_scope FOREIGN KEY (tenant, event_id) REFERENCES public.outbox(tenant, id);

ALTER TABLE ONLY public.app_webhook_receipts FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.auth_attempt_buckets
    ADD CONSTRAINT auth_attempt_buckets_pkey PRIMARY KEY (key);

ALTER TABLE ONLY public.b2b_approvals
    ADD CONSTRAINT b2b_approvals_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.b2b_approvals
    ADD CONSTRAINT b2b_approvals_tenant_cart_id_fingerprint_key UNIQUE (tenant, cart_id, fingerprint);

ALTER TABLE ONLY public.b2b_approvals
    ADD CONSTRAINT b2b_approvals_tenant_cart_id_fkey FOREIGN KEY (tenant, cart_id) REFERENCES public.carts(tenant, id);

ALTER TABLE ONLY public.b2b_approvals
    ADD CONSTRAINT b2b_approvals_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.b2b_approvals
    ADD CONSTRAINT b2b_approvals_tenant_unit_id_buyer_id_fkey FOREIGN KEY (tenant, unit_id, buyer_id) REFERENCES public.b2b_members(tenant, unit_id, customer_id);

ALTER TABLE ONLY public.b2b_approvals FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.b2b_budget_allocations
    ADD CONSTRAINT b2b_budget_allocations_pkey PRIMARY KEY (tenant, order_id);

ALTER TABLE ONLY public.b2b_budget_allocations
    ADD CONSTRAINT b2b_budget_allocations_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.b2b_budget_allocations
    ADD CONSTRAINT b2b_budget_allocations_tenant_order_id_fkey FOREIGN KEY (tenant, order_id) REFERENCES public.orders(tenant, id);

ALTER TABLE ONLY public.b2b_budget_allocations
    ADD CONSTRAINT b2b_budget_allocations_tenant_unit_id_fkey FOREIGN KEY (tenant, unit_id) REFERENCES public.b2b_units(tenant, id);

ALTER TABLE ONLY public.b2b_budget_allocations FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.b2b_catalog
    ADD CONSTRAINT b2b_catalog_pkey PRIMARY KEY (tenant, unit_id, product_id);

ALTER TABLE ONLY public.b2b_catalog
    ADD CONSTRAINT b2b_catalog_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.b2b_catalog
    ADD CONSTRAINT b2b_catalog_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.b2b_catalog
    ADD CONSTRAINT b2b_catalog_tenant_unit_id_fkey FOREIGN KEY (tenant, unit_id) REFERENCES public.b2b_units(tenant, id);

ALTER TABLE ONLY public.b2b_catalog FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.b2b_companies
    ADD CONSTRAINT b2b_companies_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.b2b_companies
    ADD CONSTRAINT b2b_companies_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.b2b_companies FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.b2b_lists
    ADD CONSTRAINT b2b_lists_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.b2b_lists
    ADD CONSTRAINT b2b_lists_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.b2b_lists
    ADD CONSTRAINT b2b_lists_tenant_unit_id_owner_id_fkey FOREIGN KEY (tenant, unit_id, owner_id) REFERENCES public.b2b_members(tenant, unit_id, customer_id);

ALTER TABLE ONLY public.b2b_lists FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.b2b_members
    ADD CONSTRAINT b2b_members_pkey PRIMARY KEY (tenant, unit_id, customer_id);

ALTER TABLE ONLY public.b2b_members
    ADD CONSTRAINT b2b_members_tenant_customer_id_fkey FOREIGN KEY (tenant, customer_id) REFERENCES public.customers(tenant, id);

ALTER TABLE ONLY public.b2b_members
    ADD CONSTRAINT b2b_members_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.b2b_members
    ADD CONSTRAINT b2b_members_tenant_unit_id_fkey FOREIGN KEY (tenant, unit_id) REFERENCES public.b2b_units(tenant, id);

ALTER TABLE ONLY public.b2b_members FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.b2b_quotes
    ADD CONSTRAINT b2b_quotes_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.b2b_quotes
    ADD CONSTRAINT b2b_quotes_tenant_cart_id_fkey FOREIGN KEY (tenant, cart_id) REFERENCES public.carts(tenant, id);

ALTER TABLE ONLY public.b2b_quotes
    ADD CONSTRAINT b2b_quotes_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.b2b_quotes
    ADD CONSTRAINT b2b_quotes_tenant_unit_id_buyer_id_fkey FOREIGN KEY (tenant, unit_id, buyer_id) REFERENCES public.b2b_members(tenant, unit_id, customer_id);

ALTER TABLE ONLY public.b2b_quotes FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.b2b_units
    ADD CONSTRAINT b2b_units_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.b2b_units
    ADD CONSTRAINT b2b_units_tenant_company_id_fkey FOREIGN KEY (tenant, company_id) REFERENCES public.b2b_companies(tenant, id);

ALTER TABLE ONLY public.b2b_units
    ADD CONSTRAINT b2b_units_tenant_company_id_id_key UNIQUE (tenant, company_id, id);

ALTER TABLE ONLY public.b2b_units
    ADD CONSTRAINT b2b_units_tenant_company_id_parent_id_fkey FOREIGN KEY (tenant, company_id, parent_id) REFERENCES public.b2b_units(tenant, company_id, id);

ALTER TABLE ONLY public.b2b_units
    ADD CONSTRAINT b2b_units_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.b2b_units FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.carts
    ADD CONSTRAINT carts_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.carts
    ADD CONSTRAINT carts_sales_channel_tenant_fk FOREIGN KEY (tenant, sales_channel_id) REFERENCES public.sales_channels(tenant, id);

ALTER TABLE ONLY public.carts
    ADD CONSTRAINT carts_tenant_id_key UNIQUE (tenant, id);

ALTER TABLE ONLY public.carts
    ADD CONSTRAINT carts_token_key UNIQUE (token);

ALTER TABLE ONLY public.carts FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.categories
    ADD CONSTRAINT categories_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.categories
    ADD CONSTRAINT categories_tenant_parent_id_fkey FOREIGN KEY (tenant, parent_id) REFERENCES public.categories(tenant, id);

ALTER TABLE ONLY public.categories FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.channel_metrics
    ADD CONSTRAINT channel_metrics_pkey PRIMARY KEY (tenant, channel);

ALTER TABLE ONLY public.channel_metrics FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.channel_previews
    ADD CONSTRAINT channel_previews_pkey PRIMARY KEY (digest);

ALTER TABLE ONLY public.channel_previews
    ADD CONSTRAINT channel_previews_session_digest_fkey FOREIGN KEY (session_digest) REFERENCES public.user_sessions(digest) ON DELETE CASCADE;

ALTER TABLE ONLY public.channel_previews
    ADD CONSTRAINT channel_previews_tenant_channel_fkey FOREIGN KEY (tenant, channel) REFERENCES public.sales_channels(tenant, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.channel_previews FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.chat_messages
    ADD CONSTRAINT chat_messages_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.chat_messages
    ADD CONSTRAINT chat_messages_tenant_conversation_id_fkey FOREIGN KEY (tenant, conversation_id) REFERENCES public.conversations(tenant, id);

ALTER TABLE ONLY public.chat_messages ALTER COLUMN id SET DEFAULT nextval('public.chat_messages_id_seq'::regclass);

ALTER TABLE ONLY public.chat_messages FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.checkout_handoffs
    ADD CONSTRAINT checkout_handoffs_cart_id_fkey FOREIGN KEY (cart_id) REFERENCES public.carts(id);

ALTER TABLE ONLY public.checkout_handoffs
    ADD CONSTRAINT checkout_handoffs_cart_id_key UNIQUE (cart_id);

ALTER TABLE ONLY public.checkout_handoffs
    ADD CONSTRAINT checkout_handoffs_pkey PRIMARY KEY (digest);

ALTER TABLE ONLY public.checkout_handoffs
    ADD CONSTRAINT checkout_handoffs_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.checkout_handoffs
    ADD CONSTRAINT handoffs_cart_scope FOREIGN KEY (tenant, cart_id) REFERENCES public.carts(tenant, id);

ALTER TABLE ONLY public.checkout_handoffs FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.commerce_customer_groups
    ADD CONSTRAINT commerce_customer_groups_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.commerce_customer_groups FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.commerce_flows
    ADD CONSTRAINT commerce_flows_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.commerce_flows FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.commerce_migrations
    ADD CONSTRAINT commerce_migrations_pkey PRIMARY KEY (version);

ALTER TABLE ONLY public.commerce_overrides
    ADD CONSTRAINT commerce_overrides_pkey PRIMARY KEY (tenant, channel_id);

ALTER TABLE ONLY public.commerce_overrides
    ADD CONSTRAINT commerce_overrides_tenant_channel_id_fkey FOREIGN KEY (tenant, channel_id) REFERENCES public.sales_channels(tenant, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.commerce_overrides FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.commerce_promotions
    ADD CONSTRAINT commerce_promotions_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.commerce_promotions FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.commerce_rules
    ADD CONSTRAINT commerce_rules_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.commerce_rules FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.commerce_settings
    ADD CONSTRAINT commerce_settings_pkey PRIMARY KEY (tenant);

ALTER TABLE ONLY public.commerce_settings FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.company_logos
    ADD CONSTRAINT company_logos_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.company_logos
    ADD CONSTRAINT company_logos_tenant_digest_key UNIQUE (tenant, digest);

ALTER TABLE ONLY public.company_logos
    ADD CONSTRAINT company_logos_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.company_logos FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.company_overrides
    ADD CONSTRAINT company_overrides_pkey PRIMARY KEY (tenant, channel_id);

ALTER TABLE ONLY public.company_overrides
    ADD CONSTRAINT company_overrides_tenant_channel_id_fkey FOREIGN KEY (tenant, channel_id) REFERENCES public.sales_channels(tenant, id);

ALTER TABLE ONLY public.company_overrides FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.connector_changes
    ADD CONSTRAINT connector_changes_pkey PRIMARY KEY (seq);

ALTER TABLE ONLY public.connector_changes
    ADD CONSTRAINT connector_changes_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.connector_changes FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.connector_config
    ADD CONSTRAINT connector_config_pkey PRIMARY KEY (tenant, app);

ALTER TABLE ONLY public.connector_config
    ADD CONSTRAINT connector_config_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.connector_config FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.connector_jobs
    ADD CONSTRAINT connector_jobs_pkey PRIMARY KEY (tenant, app, id);

ALTER TABLE ONLY public.connector_jobs
    ADD CONSTRAINT connector_jobs_tenant_app_fkey FOREIGN KEY (tenant, app) REFERENCES public.connector_limits(tenant, app) ON DELETE CASCADE;

ALTER TABLE ONLY public.connector_jobs FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.connector_limits
    ADD CONSTRAINT connector_limits_pkey PRIMARY KEY (tenant, app);

ALTER TABLE ONLY public.connector_limits
    ADD CONSTRAINT connector_limits_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.connector_limits FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.connector_oauth
    ADD CONSTRAINT connector_oauth_pkey PRIMARY KEY (digest);

ALTER TABLE ONLY public.connector_oauth
    ADD CONSTRAINT connector_oauth_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.connector_oauth FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.connector_sources
    ADD CONSTRAINT connector_sources_pkey PRIMARY KEY (tenant, app, id);

ALTER TABLE ONLY public.connector_sources
    ADD CONSTRAINT connector_sources_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.connector_sources FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.consumer_request_reviews
    ADD CONSTRAINT consumer_request_reviews_pkey PRIMARY KEY (tenant, request_id, revision);

ALTER TABLE ONLY public.consumer_request_reviews
    ADD CONSTRAINT consumer_request_reviews_tenant_request_id_fkey FOREIGN KEY (tenant, request_id) REFERENCES public.consumer_requests(tenant, id);

ALTER TABLE ONLY public.consumer_request_reviews FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.consumer_requests
    ADD CONSTRAINT consumer_requests_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.consumer_requests
    ADD CONSTRAINT consumer_requests_tenant_cart_id_fkey FOREIGN KEY (tenant, cart_id) REFERENCES public.carts(tenant, id);

ALTER TABLE ONLY public.consumer_requests
    ADD CONSTRAINT consumer_requests_tenant_channel_id_fkey FOREIGN KEY (tenant, channel_id) REFERENCES public.sales_channels(tenant, id);

ALTER TABLE ONLY public.consumer_requests
    ADD CONSTRAINT consumer_requests_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.consumer_requests FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.conversations
    ADD CONSTRAINT conversations_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.conversations FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.currency_price_jobs
    ADD CONSTRAINT currency_price_jobs_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.currency_price_jobs
    ADD CONSTRAINT currency_price_jobs_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.currency_price_jobs FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.customer_addresses
    ADD CONSTRAINT customer_addresses_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.customer_addresses
    ADD CONSTRAINT customer_addresses_tenant_email_fkey FOREIGN KEY (tenant, email) REFERENCES public.customers(tenant, email) ON DELETE CASCADE;

ALTER TABLE ONLY public.customer_addresses
    ADD CONSTRAINT customer_addresses_tenant_email_id_key UNIQUE (tenant, email, id);

ALTER TABLE ONLY public.customer_addresses FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.customer_sessions
    ADD CONSTRAINT customer_sessions_pkey PRIMARY KEY (digest);

ALTER TABLE ONLY public.customer_sessions
    ADD CONSTRAINT customer_sessions_tenant_email_fkey FOREIGN KEY (tenant, email) REFERENCES public.customers(tenant, email) ON DELETE CASCADE;

ALTER TABLE ONLY public.customer_sessions FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.customers
    ADD CONSTRAINT customers_pkey PRIMARY KEY (tenant, email);

ALTER TABLE ONLY public.customers
    ADD CONSTRAINT customers_sales_channel_tenant_fk FOREIGN KEY (tenant, sales_channel_id) REFERENCES public.sales_channels(tenant, id);

ALTER TABLE ONLY public.customers
    ADD CONSTRAINT customers_tenant_email_default_billing_address_id_fkey FOREIGN KEY (tenant, email, default_billing_address_id) REFERENCES public.customer_addresses(tenant, email, id) ON DELETE SET NULL (default_billing_address_id);

ALTER TABLE ONLY public.customers
    ADD CONSTRAINT customers_tenant_email_default_shipping_address_id_fkey FOREIGN KEY (tenant, email, default_shipping_address_id) REFERENCES public.customer_addresses(tenant, email, id) ON DELETE SET NULL (default_shipping_address_id);

ALTER TABLE ONLY public.customers
    ADD CONSTRAINT customers_tenant_id UNIQUE (tenant, id);

ALTER TABLE ONLY public.customers
    ADD CONSTRAINT customers_tenant_number UNIQUE (tenant, customer_number);

ALTER TABLE ONLY public.customers FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.developer_builds
    ADD CONSTRAINT builds_environment_scope FOREIGN KEY (tenant, environment) REFERENCES public.shop_environments(live_tenant, tenant);

ALTER TABLE ONLY public.developer_builds
    ADD CONSTRAINT developer_builds_environment_fkey FOREIGN KEY (environment) REFERENCES public.shop_environments(tenant);

ALTER TABLE ONLY public.developer_builds
    ADD CONSTRAINT developer_builds_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.developer_builds
    ADD CONSTRAINT developer_builds_tenant_app_version_key UNIQUE (tenant, app, version);

ALTER TABLE ONLY public.developer_builds
    ADD CONSTRAINT developer_builds_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.developer_builds FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.developer_drafts
    ADD CONSTRAINT developer_drafts_environment_tenant_fkey FOREIGN KEY (environment, tenant) REFERENCES public.shop_environments(tenant, live_tenant);

ALTER TABLE ONLY public.developer_drafts
    ADD CONSTRAINT developer_drafts_pkey PRIMARY KEY (tenant, actor_key, id);

ALTER TABLE ONLY public.developer_drafts
    ADD CONSTRAINT developer_drafts_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.developer_drafts FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.embedding_generations
    ADD CONSTRAINT embedding_generations_pkey PRIMARY KEY (tenant);

ALTER TABLE ONLY public.embedding_generations
    ADD CONSTRAINT embedding_generations_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.embedding_generations FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.embedding_jobs
    ADD CONSTRAINT embedding_jobs_pkey PRIMARY KEY (tenant, kind, object_id);

ALTER TABLE ONLY public.embedding_jobs
    ADD CONSTRAINT embedding_jobs_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.embedding_jobs FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.entity_history
    ADD CONSTRAINT entity_history_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.entity_history
    ADD CONSTRAINT entity_history_tenant_entity_entity_id_transaction_id_key UNIQUE (tenant, entity, entity_id, transaction_id);

ALTER TABLE ONLY public.entity_history ALTER COLUMN id SET DEFAULT nextval('public.entity_history_id_seq'::regclass);

ALTER TABLE ONLY public.entity_history FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.experiences
    ADD CONSTRAINT experiences_pkey PRIMARY KEY (tenant);

ALTER TABLE ONLY public.experiences FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.exposures
    ADD CONSTRAINT exposures_pkey PRIMARY KEY (tenant, session);

ALTER TABLE ONLY public.exposures FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.extensions
    ADD CONSTRAINT extensions_pkey PRIMARY KEY (tenant);

ALTER TABLE ONLY public.extensions FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.flow_jobs
    ADD CONSTRAINT flow_jobs_event_id_fkey FOREIGN KEY (event_id) REFERENCES public.outbox(id);

ALTER TABLE ONLY public.flow_jobs
    ADD CONSTRAINT flow_jobs_event_scope FOREIGN KEY (tenant, event_id) REFERENCES public.outbox(tenant, id);

ALTER TABLE ONLY public.flow_jobs
    ADD CONSTRAINT flow_jobs_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.flow_jobs
    ADD CONSTRAINT flow_jobs_tenant_flow_event_id_key UNIQUE (tenant, flow, event_id);

ALTER TABLE ONLY public.flow_jobs FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.flow_steps
    ADD CONSTRAINT flow_steps_job_fkey FOREIGN KEY (job) REFERENCES public.flow_jobs(id);

ALTER TABLE ONLY public.flow_steps
    ADD CONSTRAINT flow_steps_pkey PRIMARY KEY (job, node);

ALTER TABLE ONLY public.hosted_frontends
    ADD CONSTRAINT hosted_frontends_app_fk FOREIGN KEY (tenant, app_id) REFERENCES public.app_packages(tenant, id) ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED;

ALTER TABLE ONLY public.hosted_frontends
    ADD CONSTRAINT hosted_frontends_pkey PRIMARY KEY (alias);

ALTER TABLE ONLY public.hosted_frontends
    ADD CONSTRAINT hosted_frontends_tenant_channel_fkey FOREIGN KEY (tenant, channel) REFERENCES public.sales_channels(tenant, id) ON DELETE RESTRICT;

ALTER TABLE ONLY public.hosted_frontends FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.identity_inference_daily
    ADD CONSTRAINT identity_inference_daily_pkey PRIMARY KEY (email, day);

ALTER TABLE ONLY public.identity_nonces
    ADD CONSTRAINT identity_nonces_pkey PRIMARY KEY (digest);

ALTER TABLE ONLY public.integration_keys
    ADD CONSTRAINT integration_app_scope FOREIGN KEY (tenant, app_id) REFERENCES public.app_packages(tenant, id);

ALTER TABLE ONLY public.integration_keys
    ADD CONSTRAINT integration_keys_digest_key UNIQUE (digest);

ALTER TABLE ONLY public.integration_keys
    ADD CONSTRAINT integration_keys_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.integration_keys
    ADD CONSTRAINT integration_keys_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.integration_keys
    ADD CONSTRAINT integration_keys_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.merchant_users(id);

ALTER TABLE ONLY public.integration_keys FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.intelligence_assignments
    ADD CONSTRAINT intelligence_assignments_pkey PRIMARY KEY (tenant, experiment_id, cart_id);

ALTER TABLE ONLY public.intelligence_assignments
    ADD CONSTRAINT intelligence_assignments_tenant_cart_id_fkey FOREIGN KEY (tenant, cart_id) REFERENCES public.carts(tenant, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.intelligence_assignments
    ADD CONSTRAINT intelligence_assignments_tenant_experiment_id_fkey FOREIGN KEY (tenant, experiment_id) REFERENCES public.intelligence_experiments(tenant, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.intelligence_assignments FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.intelligence_experiments
    ADD CONSTRAINT intelligence_experiments_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.intelligence_experiments
    ADD CONSTRAINT intelligence_experiments_tenant_channel_fkey FOREIGN KEY (tenant, channel) REFERENCES public.sales_channels(tenant, id);

ALTER TABLE ONLY public.intelligence_experiments
    ADD CONSTRAINT intelligence_experiments_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.intelligence_experiments FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.inventory_reservations
    ADD CONSTRAINT inventory_reservations_attempt_id_fkey FOREIGN KEY (attempt_id) REFERENCES public.payment_attempts(id);

ALTER TABLE ONLY public.inventory_reservations
    ADD CONSTRAINT inventory_reservations_pkey PRIMARY KEY (tenant, attempt_id, product_id);

ALTER TABLE ONLY public.inventory_reservations
    ADD CONSTRAINT inventory_reservations_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.inventory_reservations
    ADD CONSTRAINT reservations_attempt_scope FOREIGN KEY (tenant, attempt_id) REFERENCES public.payment_attempts(tenant, id);

ALTER TABLE ONLY public.inventory_reservations FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.knowledge_chunk_lexemes
    ADD CONSTRAINT knowledge_chunk_lexemes_pkey PRIMARY KEY (tenant, lexeme, document_id, "position");

ALTER TABLE ONLY public.knowledge_chunk_lexemes
    ADD CONSTRAINT knowledge_chunk_lexemes_tenant_document_id_position_fkey FOREIGN KEY (tenant, document_id, "position") REFERENCES public.knowledge_chunks(tenant, document_id, "position") ON DELETE CASCADE;

ALTER TABLE ONLY public.knowledge_chunk_lexemes FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.knowledge_chunks
    ADD CONSTRAINT knowledge_chunks_pkey PRIMARY KEY (tenant, document_id, "position");

ALTER TABLE ONLY public.knowledge_chunks
    ADD CONSTRAINT knowledge_chunks_tenant_document_id_fkey FOREIGN KEY (tenant, document_id) REFERENCES public.knowledge_documents(tenant, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.knowledge_chunks FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.knowledge_documents
    ADD CONSTRAINT knowledge_documents_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.knowledge_documents
    ADD CONSTRAINT knowledge_documents_tenant_product_id_content_hash_key UNIQUE NULLS NOT DISTINCT (tenant, product_id, content_hash);

ALTER TABLE ONLY public.knowledge_documents
    ADD CONSTRAINT knowledge_documents_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.knowledge_documents FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.knowledge_hypotheses
    ADD CONSTRAINT knowledge_hypotheses_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.knowledge_hypotheses FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.knowledge_index_status
    ADD CONSTRAINT knowledge_index_status_pkey PRIMARY KEY (tenant);

ALTER TABLE ONLY public.knowledge_index_status
    ADD CONSTRAINT knowledge_index_status_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.knowledge_index_status FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.knowledge_nodes
    ADD CONSTRAINT knowledge_nodes_pkey PRIMARY KEY (tenant, kind, id);

ALTER TABLE ONLY public.knowledge_nodes
    ADD CONSTRAINT knowledge_nodes_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.knowledge_nodes FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.knowledge_product_lexemes
    ADD CONSTRAINT knowledge_product_lexemes_pkey PRIMARY KEY (tenant, lexeme, product_id);

ALTER TABLE ONLY public.knowledge_product_lexemes
    ADD CONSTRAINT knowledge_product_lexemes_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.knowledge_product_lexemes FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.knowledge_receipts
    ADD CONSTRAINT knowledge_receipts_event_id_fkey FOREIGN KEY (event_id) REFERENCES public.outbox(id);

ALTER TABLE ONLY public.knowledge_receipts
    ADD CONSTRAINT knowledge_receipts_event_scope FOREIGN KEY (tenant, event_id) REFERENCES public.outbox(tenant, id);

ALTER TABLE ONLY public.knowledge_receipts
    ADD CONSTRAINT knowledge_receipts_pkey PRIMARY KEY (tenant, event_id);

ALTER TABLE ONLY public.knowledge_receipts FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.knowledge_relation_history
    ADD CONSTRAINT knowledge_relation_history_pkey PRIMARY KEY (tenant, kind, source_id, target_id, revision);

ALTER TABLE ONLY public.knowledge_relation_history
    ADD CONSTRAINT knowledge_relation_history_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.knowledge_relation_history FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.knowledge_relations
    ADD CONSTRAINT knowledge_relations_pkey PRIMARY KEY (tenant, kind, source_id, target_id);

ALTER TABLE ONLY public.knowledge_relations
    ADD CONSTRAINT knowledge_relations_source_node FOREIGN KEY (tenant, source_kind, source_id) REFERENCES public.knowledge_nodes(tenant, kind, id);

ALTER TABLE ONLY public.knowledge_relations
    ADD CONSTRAINT knowledge_relations_target_node FOREIGN KEY (tenant, target_kind, target_id) REFERENCES public.knowledge_nodes(tenant, kind, id);

ALTER TABLE ONLY public.knowledge_relations
    ADD CONSTRAINT knowledge_relations_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.knowledge_relations FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.languages
    ADD CONSTRAINT languages_locale_key UNIQUE (locale);

ALTER TABLE ONLY public.languages
    ADD CONSTRAINT languages_parent_id_fkey FOREIGN KEY (parent_id) REFERENCES public.languages(id);

ALTER TABLE ONLY public.languages
    ADD CONSTRAINT languages_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.legal_acceptances
    ADD CONSTRAINT legal_acceptances_pkey PRIMARY KEY (tenant, cart_id);

ALTER TABLE ONLY public.legal_acceptances
    ADD CONSTRAINT legal_acceptances_tenant_cart_id_fkey FOREIGN KEY (tenant, cart_id) REFERENCES public.carts(tenant, id);

ALTER TABLE ONLY public.legal_acceptances
    ADD CONSTRAINT legal_acceptances_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.legal_acceptances FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.media_jobs
    ADD CONSTRAINT media_jobs_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.media_jobs
    ADD CONSTRAINT media_jobs_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.media_jobs FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.memberships
    ADD CONSTRAINT memberships_pkey PRIMARY KEY (user_id, tenant);

ALTER TABLE ONLY public.memberships
    ADD CONSTRAINT memberships_role_tenant_fk FOREIGN KEY (tenant, role) REFERENCES public.workspace_roles(tenant, id);

ALTER TABLE ONLY public.memberships
    ADD CONSTRAINT memberships_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.memberships
    ADD CONSTRAINT memberships_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.merchant_users(id);

ALTER TABLE ONLY public.memberships FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.merchant_handoffs
    ADD CONSTRAINT merchant_handoffs_pkey PRIMARY KEY (digest);

ALTER TABLE ONLY public.merchant_handoffs
    ADD CONSTRAINT merchant_handoffs_session_digest_fkey FOREIGN KEY (session_digest) REFERENCES public.user_sessions(digest) ON DELETE CASCADE;

ALTER TABLE ONLY public.merchant_handoffs
    ADD CONSTRAINT merchant_handoffs_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.merchant_handoffs
    ADD CONSTRAINT merchant_handoffs_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.merchant_users(id);

ALTER TABLE ONLY public.merchant_handoffs FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.merchant_identities
    ADD CONSTRAINT merchant_identities_pkey PRIMARY KEY (issuer, subject);

ALTER TABLE ONLY public.merchant_identities
    ADD CONSTRAINT merchant_identities_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.merchant_users(id);

ALTER TABLE ONLY public.merchant_mfa
    ADD CONSTRAINT merchant_mfa_pkey PRIMARY KEY (user_id);

ALTER TABLE ONLY public.merchant_mfa
    ADD CONSTRAINT merchant_mfa_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.merchant_users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.merchant_mfa FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.merchant_mfa_challenges
    ADD CONSTRAINT merchant_mfa_challenges_pkey PRIMARY KEY (digest);

ALTER TABLE ONLY public.merchant_mfa_challenges
    ADD CONSTRAINT merchant_mfa_challenges_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.merchant_mfa_challenges
    ADD CONSTRAINT merchant_mfa_challenges_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.merchant_users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.merchant_mfa_challenges FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.merchant_users
    ADD CONSTRAINT merchant_users_email_key UNIQUE (email);

ALTER TABLE ONLY public.merchant_users
    ADD CONSTRAINT merchant_users_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.merchant_users FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.observed_pairs
    ADD CONSTRAINT observed_pairs_event_scope FOREIGN KEY (tenant, last_event) REFERENCES public.outbox(tenant, id);

ALTER TABLE ONLY public.observed_pairs
    ADD CONSTRAINT observed_pairs_last_event_fkey FOREIGN KEY (last_event) REFERENCES public.outbox(id);

ALTER TABLE ONLY public.observed_pairs
    ADD CONSTRAINT observed_pairs_pkey PRIMARY KEY (tenant, left_id, right_id);

ALTER TABLE ONLY public.observed_pairs FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.order_activity
    ADD CONSTRAINT order_activity_order_id_fkey FOREIGN KEY (order_id) REFERENCES public.orders(id);

ALTER TABLE ONLY public.order_activity
    ADD CONSTRAINT order_activity_order_scope FOREIGN KEY (tenant, order_id) REFERENCES public.orders(tenant, id);

ALTER TABLE ONLY public.order_activity
    ADD CONSTRAINT order_activity_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.order_activity ALTER COLUMN id SET DEFAULT nextval('public.order_activity_id_seq'::regclass);

ALTER TABLE ONLY public.order_activity FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.order_downloads
    ADD CONSTRAINT downloads_order_scope FOREIGN KEY (tenant, order_id) REFERENCES public.orders(tenant, id);

ALTER TABLE ONLY public.order_downloads
    ADD CONSTRAINT order_downloads_order_id_fkey FOREIGN KEY (order_id) REFERENCES public.orders(id);

ALTER TABLE ONLY public.order_downloads
    ADD CONSTRAINT order_downloads_pkey PRIMARY KEY (tenant, order_id, asset_id);

ALTER TABLE ONLY public.order_downloads
    ADD CONSTRAINT order_downloads_tenant_asset_id_fkey FOREIGN KEY (tenant, asset_id) REFERENCES public.product_assets(tenant, id);

ALTER TABLE ONLY public.order_downloads FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.order_inventory_reservations
    ADD CONSTRAINT order_inventory_reservations_pkey PRIMARY KEY (tenant, order_id, product_id);

ALTER TABLE ONLY public.order_inventory_reservations
    ADD CONSTRAINT order_inventory_reservations_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.order_inventory_reservations
    ADD CONSTRAINT order_inventory_reservations_tenant_order_id_fkey FOREIGN KEY (tenant, order_id) REFERENCES public.orders(tenant, id);

ALTER TABLE ONLY public.order_inventory_reservations
    ADD CONSTRAINT order_inventory_reservations_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.order_inventory_reservations FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.order_receipts
    ADD CONSTRAINT order_document_app_scope FOREIGN KEY (tenant, source_app) REFERENCES public.app_packages(tenant, id);

ALTER TABLE ONLY public.order_receipts
    ADD CONSTRAINT order_receipts_order_id_fkey FOREIGN KEY (order_id) REFERENCES public.orders(id);

ALTER TABLE ONLY public.order_receipts
    ADD CONSTRAINT order_receipts_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.order_receipts
    ADD CONSTRAINT order_receipts_tenant_number_key UNIQUE (tenant, number);

ALTER TABLE ONLY public.order_receipts
    ADD CONSTRAINT order_receipts_tenant_request_key_key UNIQUE (tenant, request_key);

ALTER TABLE ONLY public.order_receipts
    ADD CONSTRAINT receipts_order_scope FOREIGN KEY (tenant, order_id) REFERENCES public.orders(tenant, id);

ALTER TABLE ONLY public.order_receipts FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.order_state_machines
    ADD CONSTRAINT order_state_machines_pkey PRIMARY KEY (tenant);

ALTER TABLE ONLY public.order_state_machines
    ADD CONSTRAINT order_state_machines_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.order_state_machines FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.order_transition_requests
    ADD CONSTRAINT order_transition_requests_order_id_fkey FOREIGN KEY (order_id) REFERENCES public.orders(id);

ALTER TABLE ONLY public.order_transition_requests
    ADD CONSTRAINT order_transition_requests_pkey PRIMARY KEY (tenant, request_key);

ALTER TABLE ONLY public.order_transition_requests
    ADD CONSTRAINT order_transition_requests_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.order_transition_requests
    ADD CONSTRAINT transition_order_scope FOREIGN KEY (tenant, order_id) REFERENCES public.orders(tenant, id);

ALTER TABLE ONLY public.order_transition_requests FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.orders
    ADD CONSTRAINT orders_cart_id_fkey FOREIGN KEY (cart_id) REFERENCES public.carts(id);

ALTER TABLE ONLY public.orders
    ADD CONSTRAINT orders_cart_id_key UNIQUE (cart_id);

ALTER TABLE ONLY public.orders
    ADD CONSTRAINT orders_cart_scope FOREIGN KEY (tenant, cart_id) REFERENCES public.carts(tenant, id);

ALTER TABLE ONLY public.orders
    ADD CONSTRAINT orders_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.orders
    ADD CONSTRAINT orders_tenant_id_key UNIQUE (tenant, id);

ALTER TABLE ONLY public.orders
    ADD CONSTRAINT orders_tenant_idempotency_key_key UNIQUE (tenant, idempotency_key);

ALTER TABLE ONLY public.orders FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.outbox
    ADD CONSTRAINT outbox_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.outbox
    ADD CONSTRAINT outbox_tenant_id_key UNIQUE (tenant, id);

ALTER TABLE ONLY public.outbox ALTER COLUMN id SET DEFAULT nextval('public.outbox_id_seq'::regclass);

ALTER TABLE ONLY public.outbox FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.outbox_checkpoints
    ADD CONSTRAINT outbox_checkpoints_pkey PRIMARY KEY (tenant, subscriber);

ALTER TABLE ONLY public.outbox_checkpoints
    ADD CONSTRAINT outbox_checkpoints_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.outbox_checkpoints FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.outbox_subscriber_failures
    ADD CONSTRAINT outbox_subscriber_failures_pkey PRIMARY KEY (tenant, event_id, subscriber);

ALTER TABLE ONLY public.outbox_subscriber_failures
    ADD CONSTRAINT outbox_subscriber_failures_tenant_event_id_fkey FOREIGN KEY (tenant, event_id) REFERENCES public.outbox(tenant, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.outbox_subscriber_failures FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.outbox_tenant_dispatch
    ADD CONSTRAINT outbox_tenant_dispatch_pkey PRIMARY KEY (tenant);

ALTER TABLE ONLY public.outbox_tenant_dispatch
    ADD CONSTRAINT outbox_tenant_dispatch_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.outbox_tenant_dispatch FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.pair_evidence
    ADD CONSTRAINT pair_evidence_event_id_fkey FOREIGN KEY (event_id) REFERENCES public.outbox(id);

ALTER TABLE ONLY public.pair_evidence
    ADD CONSTRAINT pair_evidence_event_scope FOREIGN KEY (tenant, event_id) REFERENCES public.outbox(tenant, id);

ALTER TABLE ONLY public.pair_evidence
    ADD CONSTRAINT pair_evidence_pkey PRIMARY KEY (tenant, left_id, right_id, order_id);

ALTER TABLE ONLY public.pair_evidence FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.payment_attempts
    ADD CONSTRAINT payment_attempts_order_id_fkey FOREIGN KEY (order_id) REFERENCES public.orders(id);

ALTER TABLE ONLY public.payment_attempts
    ADD CONSTRAINT payment_attempts_order_id_key UNIQUE (order_id);

ALTER TABLE ONLY public.payment_attempts
    ADD CONSTRAINT payment_attempts_order_scope FOREIGN KEY (tenant, order_id) REFERENCES public.orders(tenant, id);

ALTER TABLE ONLY public.payment_attempts
    ADD CONSTRAINT payment_attempts_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.payment_attempts
    ADD CONSTRAINT payment_attempts_tenant_id_key UNIQUE (tenant, id);

ALTER TABLE ONLY public.payment_attempts FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.payment_inbox
    ADD CONSTRAINT payment_inbox_pkey PRIMARY KEY (tenant, provider, event_id);

ALTER TABLE ONLY public.payment_inbox FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.payment_instruments
    ADD CONSTRAINT payment_instruments_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.payment_instruments
    ADD CONSTRAINT payment_instruments_tenant_attempt_id_fkey FOREIGN KEY (tenant, attempt_id) REFERENCES public.payment_attempts(tenant, id);

ALTER TABLE ONLY public.payment_instruments
    ADD CONSTRAINT payment_instruments_tenant_customer_id_fkey FOREIGN KEY (tenant, customer_id) REFERENCES public.customers(tenant, id);

ALTER TABLE ONLY public.payment_instruments
    ADD CONSTRAINT payment_instruments_tenant_provider_environment_account_ref_key UNIQUE (tenant, provider, environment, account_ref, reference);

ALTER TABLE ONLY public.payment_instruments FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.payment_jobs
    ADD CONSTRAINT payment_jobs_attempt_id_fkey FOREIGN KEY (attempt_id) REFERENCES public.payment_attempts(id);

ALTER TABLE ONLY public.payment_jobs
    ADD CONSTRAINT payment_jobs_attempt_scope FOREIGN KEY (tenant, attempt_id) REFERENCES public.payment_attempts(tenant, id);

ALTER TABLE ONLY public.payment_jobs
    ADD CONSTRAINT payment_jobs_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.payment_jobs
    ADD CONSTRAINT payment_jobs_tenant_id_attempt_key UNIQUE (tenant, id, attempt_id);

ALTER TABLE ONLY public.payment_jobs
    ADD CONSTRAINT payment_jobs_tenant_id_key UNIQUE (tenant, id);

ALTER TABLE ONLY public.payment_jobs FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.payment_provider_accounts
    ADD CONSTRAINT payment_provider_accounts_pkey PRIMARY KEY (tenant, app, channel);

ALTER TABLE ONLY public.payment_provider_accounts
    ADD CONSTRAINT payment_provider_accounts_tenant_app_fkey FOREIGN KEY (tenant, app) REFERENCES public.app_packages(tenant, id);

ALTER TABLE ONLY public.payment_provider_accounts
    ADD CONSTRAINT payment_provider_accounts_tenant_channel_fkey FOREIGN KEY (tenant, channel) REFERENCES public.sales_channels(tenant, id);

ALTER TABLE ONLY public.payment_provider_accounts FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.payment_receipt_claims
    ADD CONSTRAINT payment_receipt_claims_pkey PRIMARY KEY (provider, environment, account_ref, kind, receipt_ref);

ALTER TABLE ONLY public.payment_receipt_claims
    ADD CONSTRAINT payment_receipt_claims_tenant_attempt_id_fkey FOREIGN KEY (tenant, attempt_id) REFERENCES public.payment_attempts(tenant, id);

ALTER TABLE ONLY public.payment_receipt_claims FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.payment_refunds
    ADD CONSTRAINT payment_refunds_pkey PRIMARY KEY (tenant, job_id);

ALTER TABLE ONLY public.payment_refunds
    ADD CONSTRAINT payment_refunds_tenant_provider_id_key UNIQUE (tenant, provider_id);

ALTER TABLE ONLY public.payment_refunds
    ADD CONSTRAINT refunds_job_attempt_scope FOREIGN KEY (tenant, job_id, attempt_id) REFERENCES public.payment_jobs(tenant, id, attempt_id);

ALTER TABLE ONLY public.payment_refunds FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.platform_ai
    ADD CONSTRAINT platform_ai_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.platform_ai
    ADD CONSTRAINT platform_ai_updated_by_fkey FOREIGN KEY (updated_by) REFERENCES public.merchant_users(id);

ALTER TABLE ONLY public.platform_audit
    ADD CONSTRAINT platform_audit_actor_fkey FOREIGN KEY (actor) REFERENCES public.merchant_users(id);

ALTER TABLE ONLY public.platform_audit
    ADD CONSTRAINT platform_audit_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.platform_audit ALTER COLUMN id SET DEFAULT nextval('public.platform_audit_id_seq'::regclass);

ALTER TABLE ONLY public.platform_audit FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.platform_operators
    ADD CONSTRAINT platform_operators_pkey PRIMARY KEY (user_id);

ALTER TABLE ONLY public.platform_operators
    ADD CONSTRAINT platform_operators_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.merchant_users(id);

ALTER TABLE ONLY public.policy
    ADD CONSTRAINT policy_pkey PRIMARY KEY (tenant, variant);

ALTER TABLE ONLY public.policy FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.privacy_consent_log
    ADD CONSTRAINT privacy_consent_log_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.privacy_consent_log
    ADD CONSTRAINT privacy_consent_log_tenant_cart_id_fkey FOREIGN KEY (tenant, cart_id) REFERENCES public.carts(tenant, id);

ALTER TABLE ONLY public.privacy_consent_log
    ADD CONSTRAINT privacy_consent_log_tenant_channel_id_fkey FOREIGN KEY (tenant, channel_id) REFERENCES public.sales_channels(tenant, id);

ALTER TABLE ONLY public.privacy_consent_log
    ADD CONSTRAINT privacy_consent_log_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.privacy_consent_log FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.privacy_consents
    ADD CONSTRAINT privacy_consents_pkey PRIMARY KEY (tenant, cart_id, channel_id);

ALTER TABLE ONLY public.privacy_consents
    ADD CONSTRAINT privacy_consents_source_scope FOREIGN KEY (tenant, source_cart_id, channel_id) REFERENCES public.privacy_consents(tenant, cart_id, channel_id);

ALTER TABLE ONLY public.privacy_consents
    ADD CONSTRAINT privacy_consents_tenant_cart_id_fkey FOREIGN KEY (tenant, cart_id) REFERENCES public.carts(tenant, id);

ALTER TABLE ONLY public.privacy_consents
    ADD CONSTRAINT privacy_consents_tenant_channel_id_fkey FOREIGN KEY (tenant, channel_id) REFERENCES public.sales_channels(tenant, id);

ALTER TABLE ONLY public.privacy_consents
    ADD CONSTRAINT privacy_consents_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.privacy_consents FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.private_preferences
    ADD CONSTRAINT private_preferences_pkey PRIMARY KEY (tenant, cart_id);

ALTER TABLE ONLY public.private_preferences
    ADD CONSTRAINT private_preferences_tenant_cart_id_fkey FOREIGN KEY (tenant, cart_id) REFERENCES public.carts(tenant, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.private_preferences FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.product_assets
    ADD CONSTRAINT product_assets_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.product_assets
    ADD CONSTRAINT product_assets_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.product_assets FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.product_categories
    ADD CONSTRAINT product_categories_pkey PRIMARY KEY (tenant, product_id, category_id);

ALTER TABLE ONLY public.product_categories
    ADD CONSTRAINT product_categories_tenant_category_id_fkey FOREIGN KEY (tenant, category_id) REFERENCES public.categories(tenant, id);

ALTER TABLE ONLY public.product_categories
    ADD CONSTRAINT product_categories_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.product_categories FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.product_channel_visibility
    ADD CONSTRAINT product_channel_visibility_pkey PRIMARY KEY (tenant, product_id, channel_id);

ALTER TABLE ONLY public.product_channel_visibility
    ADD CONSTRAINT product_channel_visibility_tenant_channel_id_fkey FOREIGN KEY (tenant, channel_id) REFERENCES public.sales_channels(tenant, id);

ALTER TABLE ONLY public.product_channel_visibility
    ADD CONSTRAINT product_channel_visibility_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.product_channel_visibility FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.product_reviews
    ADD CONSTRAINT product_reviews_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.product_reviews
    ADD CONSTRAINT product_reviews_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.product_reviews
    ADD CONSTRAINT product_reviews_tenant_product_id_session_key UNIQUE (tenant, product_id, session);

ALTER TABLE ONLY public.product_reviews FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.product_translations
    ADD CONSTRAINT product_translations_language_id_fkey FOREIGN KEY (language_id) REFERENCES public.languages(id);

ALTER TABLE ONLY public.product_translations
    ADD CONSTRAINT product_translations_pkey PRIMARY KEY (tenant, product_id, language_id);

ALTER TABLE ONLY public.product_translations
    ADD CONSTRAINT product_translations_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.product_translations FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.products
    ADD CONSTRAINT products_parent_scope FOREIGN KEY (tenant, parent_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.products
    ADD CONSTRAINT products_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.products FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.projections
    ADD CONSTRAINT projections_event_id_fkey FOREIGN KEY (event_id) REFERENCES public.outbox(id);

ALTER TABLE ONLY public.projections
    ADD CONSTRAINT projections_event_scope FOREIGN KEY (tenant, event_id) REFERENCES public.outbox(tenant, id);

ALTER TABLE ONLY public.projections
    ADD CONSTRAINT projections_pkey PRIMARY KEY (event_id);

ALTER TABLE ONLY public.projections FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.promotion_uses
    ADD CONSTRAINT promotion_uses_order_id_fkey FOREIGN KEY (order_id) REFERENCES public.orders(id);

ALTER TABLE ONLY public.promotion_uses
    ADD CONSTRAINT promotion_uses_order_scope FOREIGN KEY (tenant, order_id) REFERENCES public.orders(tenant, id);

ALTER TABLE ONLY public.promotion_uses
    ADD CONSTRAINT promotion_uses_pkey PRIMARY KEY (tenant, promotion, order_id);

ALTER TABLE ONLY public.promotion_uses
    ADD CONSTRAINT promotion_uses_tenant_promotion_fkey FOREIGN KEY (tenant, promotion) REFERENCES public.commerce_promotions(tenant, id);

ALTER TABLE ONLY public.promotion_uses FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.read_context_versions
    ADD CONSTRAINT read_context_versions_pkey PRIMARY KEY (namespace);

ALTER TABLE ONLY public.receipt_counters
    ADD CONSTRAINT receipt_counters_pkey PRIMARY KEY (tenant, kind);

ALTER TABLE ONLY public.receipt_counters FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.receipt_settings
    ADD CONSTRAINT receipt_settings_pkey PRIMARY KEY (tenant);

ALTER TABLE ONLY public.receipt_settings FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.resource_leases
    ADD CONSTRAINT resource_leases_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.resource_leases FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.sales_channels
    ADD CONSTRAINT sales_channels_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.sales_channels FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.semantic_products
    ADD CONSTRAINT semantic_products_pkey PRIMARY KEY (tenant, product_id);

ALTER TABLE ONLY public.semantic_products
    ADD CONSTRAINT semantic_products_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.semantic_products FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.session_signal_events
    ADD CONSTRAINT behavior_cart_fk FOREIGN KEY (tenant, cart_id) REFERENCES public.carts(tenant, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.session_signal_events
    ADD CONSTRAINT behavior_channel_fk FOREIGN KEY (tenant, channel_id) REFERENCES public.sales_channels(tenant, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.session_signal_events
    ADD CONSTRAINT behavior_customer_fk FOREIGN KEY (tenant, customer_id) REFERENCES public.customers(tenant, id) ON DELETE SET NULL (customer_id);

ALTER TABLE ONLY public.session_signal_events
    ADD CONSTRAINT behavior_product_fk FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id) ON DELETE SET NULL (product_id);

ALTER TABLE ONLY public.session_signal_events
    ADD CONSTRAINT session_signal_events_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.session_signal_events FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.session_signals
    ADD CONSTRAINT session_signals_pkey PRIMARY KEY (tenant, session, product_id);

ALTER TABLE ONLY public.session_signals
    ADD CONSTRAINT session_signals_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.session_signals FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.shop_environments
    ADD CONSTRAINT environments_live_tenant_key UNIQUE (live_tenant, tenant);

ALTER TABLE ONLY public.shop_environments
    ADD CONSTRAINT shop_environments_live_tenant_fkey FOREIGN KEY (live_tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.shop_environments
    ADD CONSTRAINT shop_environments_pkey PRIMARY KEY (tenant);

ALTER TABLE ONLY public.shop_environments
    ADD CONSTRAINT shop_environments_preview_owner_fkey FOREIGN KEY (preview_owner) REFERENCES public.merchant_users(id);

ALTER TABLE ONLY public.shop_environments
    ADD CONSTRAINT shop_environments_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.shop_environments FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.shop_releases
    ADD CONSTRAINT releases_environment_scope FOREIGN KEY (live_tenant, environment) REFERENCES public.shop_environments(live_tenant, tenant);

ALTER TABLE ONLY public.shop_releases
    ADD CONSTRAINT shop_releases_environment_fkey FOREIGN KEY (environment) REFERENCES public.shop_environments(tenant);

ALTER TABLE ONLY public.shop_releases
    ADD CONSTRAINT shop_releases_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.studio_workspace_versions
    ADD CONSTRAINT studio_workspace_versions_pkey PRIMARY KEY (tenant, user_id, revision);

ALTER TABLE ONLY public.studio_workspace_versions
    ADD CONSTRAINT studio_workspace_versions_tenant_user_id_fkey FOREIGN KEY (tenant, user_id) REFERENCES public.studio_workspaces(tenant, user_id) ON DELETE CASCADE;

ALTER TABLE ONLY public.studio_workspace_versions FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.studio_workspaces
    ADD CONSTRAINT studio_workspaces_pkey PRIMARY KEY (tenant, user_id);

ALTER TABLE ONLY public.studio_workspaces
    ADD CONSTRAINT studio_workspaces_user_id_tenant_fkey FOREIGN KEY (user_id, tenant) REFERENCES public.memberships(user_id, tenant) ON DELETE CASCADE;

ALTER TABLE ONLY public.studio_workspaces FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.tasks
    ADD CONSTRAINT tasks_conversation_scope FOREIGN KEY (tenant, conversation_id) REFERENCES public.conversations(tenant, id);

ALTER TABLE ONLY public.tasks
    ADD CONSTRAINT tasks_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.tasks
    ADD CONSTRAINT tasks_tenant_id_key UNIQUE (tenant, id);

ALTER TABLE ONLY public.tasks FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.tenant_ai_usage
    ADD CONSTRAINT tenant_ai_usage_pkey PRIMARY KEY (tenant, day);

ALTER TABLE ONLY public.tenant_ai_usage
    ADD CONSTRAINT tenant_ai_usage_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.tenant_ai_usage FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.tenant_resource_limits
    ADD CONSTRAINT tenant_resource_limits_pkey PRIMARY KEY (tenant);

ALTER TABLE ONLY public.tenant_resource_limits
    ADD CONSTRAINT tenant_resource_limits_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.tenant_resource_limits FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.tenants
    ADD CONSTRAINT tenants_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.tenants FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.translation_items
    ADD CONSTRAINT translation_items_pkey PRIMARY KEY (tenant, job_id, product_id);

ALTER TABLE ONLY public.translation_items
    ADD CONSTRAINT translation_items_tenant_job_id_fkey FOREIGN KEY (tenant, job_id) REFERENCES public.translation_jobs(tenant, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.translation_items FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.translation_jobs
    ADD CONSTRAINT translation_jobs_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.translation_jobs
    ADD CONSTRAINT translation_jobs_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.translation_jobs
    ADD CONSTRAINT translation_run_tenant_fk FOREIGN KEY (tenant, run_task) REFERENCES public.tasks(tenant, id);

ALTER TABLE ONLY public.translation_jobs FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.user_invites
    ADD CONSTRAINT user_invites_digest_key UNIQUE (digest);

ALTER TABLE ONLY public.user_invites
    ADD CONSTRAINT user_invites_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.user_invites
    ADD CONSTRAINT user_invites_role_tenant_fk FOREIGN KEY (tenant, role) REFERENCES public.workspace_roles(tenant, id);

ALTER TABLE ONLY public.user_invites
    ADD CONSTRAINT user_invites_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.user_invites FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.user_sessions
    ADD CONSTRAINT user_sessions_default_tenant_fkey FOREIGN KEY (default_tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.user_sessions
    ADD CONSTRAINT user_sessions_pkey PRIMARY KEY (digest);

ALTER TABLE ONLY public.user_sessions
    ADD CONSTRAINT user_sessions_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.merchant_users(id);

ALTER TABLE ONLY public.vector_index_queue
    ADD CONSTRAINT vector_index_queue_pkey PRIMARY KEY (tenant, kind, object_id);

ALTER TABLE ONLY public.vector_index_queue FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.workspace_roles
    ADD CONSTRAINT workspace_roles_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.workspace_roles
    ADD CONSTRAINT workspace_roles_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.workspace_roles FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY veyndo_authority.configuration
    ADD CONSTRAINT configuration_pkey PRIMARY KEY (id);

ALTER TABLE ONLY veyndo_authority.keys
    ADD CONSTRAINT keys_pkey PRIMARY KEY (id);

ALTER TABLE public.ai_price_budget ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_action_log ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_deliveries ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_evidence ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_export_cursors ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_jobs ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_models ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_packages ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_record_indexes ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_record_references ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_record_versions ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_records ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_schedule_runs ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_schedules ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_schema_history ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_service_secrets ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_storage_usage ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_surface_grants ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_versions ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_webhook_receipts ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.b2b_approvals ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.b2b_budget_allocations ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.b2b_catalog ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.b2b_companies ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.b2b_lists ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.b2b_members ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.b2b_quotes ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.b2b_units ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.carts ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.categories ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.channel_metrics ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.channel_previews ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.chat_messages ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.checkout_handoffs ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.commerce_customer_groups ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.commerce_flows ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.commerce_overrides ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.commerce_promotions ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.commerce_rules ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.commerce_settings ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.company_logos ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.company_overrides ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.connector_changes ALTER COLUMN seq ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME public.connector_changes_seq_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);

ALTER TABLE public.connector_changes ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.connector_config ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.connector_jobs ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.connector_limits ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.connector_oauth ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.connector_sources ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.consumer_request_reviews ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.consumer_requests ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.conversations ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.currency_price_jobs ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.customer_addresses ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.customer_sessions ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.customers ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.developer_builds ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.developer_drafts ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.embedding_generations ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.embedding_jobs ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.entity_history ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.experiences ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.exposures ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.extensions ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.flow_jobs ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.hosted_frontends ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.integration_keys ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.intelligence_assignments ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.intelligence_experiments ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.inventory_reservations ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.knowledge_chunk_lexemes ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.knowledge_chunks ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.knowledge_documents ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.knowledge_hypotheses ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.knowledge_index_status ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.knowledge_nodes ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.knowledge_product_lexemes ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.knowledge_receipts ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.knowledge_relation_history ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.knowledge_relations ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.legal_acceptances ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.media_jobs ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.memberships ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.merchant_handoffs ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.merchant_mfa ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.merchant_mfa_challenges ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.merchant_users ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.observed_pairs ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.order_activity ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.order_downloads ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.order_inventory_reservations ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.order_receipts ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.order_state_machines ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.order_transition_requests ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.orders ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.outbox ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.outbox_checkpoints ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.outbox_subscriber_failures ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.outbox_tenant_dispatch ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.pair_evidence ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.payment_attempts ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.payment_inbox ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.payment_instruments ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.payment_jobs ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.payment_provider_accounts ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.payment_receipt_claims ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.payment_refunds ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.platform_audit ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.policy ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.privacy_consent_log ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME public.privacy_consent_log_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);

ALTER TABLE public.privacy_consent_log ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.privacy_consents ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.private_preferences ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.product_assets ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.product_categories ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.product_channel_visibility ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.product_reviews ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.product_translations ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.products ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.projections ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.promotion_uses ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.receipt_counters ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.receipt_settings ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.resource_leases ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.sales_channels ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.semantic_products ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.session_signal_events ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.session_signals ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.shop_environments ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.studio_workspace_versions ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.studio_workspaces ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.tasks ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.tenant_ai_usage ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.tenant_resource_limits ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.tenants ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.translation_items ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.translation_jobs ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.user_invites ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.vector_index_queue ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.workspace_roles ENABLE ROW LEVEL SECURITY;

COMMENT ON EXTENSION pg_trgm IS 'text similarity measurement and index searching based on trigrams';

COMMENT ON EXTENSION pgcrypto IS 'cryptographic functions';

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.categories DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('category', 'id');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.commerce_flows DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('flow', 'id');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.commerce_overrides DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('checkoutChannel', 'channel_id');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.commerce_promotions DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('promotion', 'id');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.commerce_rules DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('rule', 'id');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.commerce_settings DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('settings', 'tenant');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.company_overrides DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('companyChannel', 'channel_id');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.customer_addresses DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('customer', 'email');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.customers DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('customer', 'email');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.knowledge_documents DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('source', 'id');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.orders DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('order', 'id');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.product_categories DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('product', 'product_id');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.product_channel_visibility DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('product', 'product_id');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.product_translations DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('product', 'product_id');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.products DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('product', 'id');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.receipt_settings DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('company', 'tenant');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.sales_channels DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('channel', 'id');

CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public;

CREATE EXTENSION IF NOT EXISTS pgcrypto WITH SCHEMA public;

CREATE FUNCTION public.app_record_prepare() RETURNS trigger
    LANGUAGE plpgsql
    SET search_path TO 'pg_catalog', 'public', 'pg_temp'
    AS $_$
DECLARE model jsonb; f jsonb; v jsonb; normalized jsonb:='{}'; target text; pos integer; relation_bytes bigint:=0; relation_rows bigint:=0;
BEGIN
 IF TG_OP='UPDATE' AND (NEW.tenant,NEW.app,NEW.entity,NEW.id) IS DISTINCT FROM (OLD.tenant,OLD.app,OLD.entity,OLD.id) THEN
  RAISE EXCEPTION 'App record identity cannot change' USING ERRCODE='42501';
 END IF;
 SELECT definition,version INTO STRICT model,NEW.version FROM app_models WHERE tenant=NEW.tenant AND app=NEW.app AND entity=NEW.entity;
 IF EXISTS(SELECT 1 FROM jsonb_object_keys(NEW.fields) k WHERE NOT EXISTS(SELECT 1 FROM jsonb_array_elements(model->'fields') AS declared(field_definition) WHERE declared.field_definition->>'name'=k)) THEN
  RAISE EXCEPTION 'Unknown app field' USING ERRCODE='23514';
 END IF;
 FOR f IN SELECT value FROM jsonb_array_elements(model->'fields') LOOP
  v:=coalesce(NEW.fields->(f->>'name'),'null'::jsonb);
  IF v='null' THEN
   IF coalesce((f->>'required')::boolean,false) THEN RAISE EXCEPTION 'Required app field missing' USING ERRCODE='23502'; END IF;
  ELSIF coalesce((f->>'translatable')::boolean,false) THEN
   IF jsonb_typeof(v)<>'object' THEN RAISE EXCEPTION 'Translation object required' USING ERRCODE='23514'; END IF;
  ELSIF f->>'kind'='integer' THEN
   IF jsonb_typeof(v)<>'number' OR v::text!~'^-?[0-9]+$' THEN RAISE EXCEPTION 'Integer required' USING ERRCODE='23514'; END IF;
   PERFORM (v::text)::bigint;
  ELSIF f->>'kind'='boolean' THEN
   IF jsonb_typeof(v)<>'boolean' THEN RAISE EXCEPTION 'Boolean required' USING ERRCODE='23514'; END IF;
  ELSIF f->>'kind'='relations' THEN
   IF jsonb_typeof(v)<>'array' OR jsonb_array_length(v)>100 THEN RAISE EXCEPTION 'Relation list required' USING ERRCODE='23514'; END IF;
   IF (SELECT count(*)<>count(DISTINCT x) FROM jsonb_array_elements(v) x) THEN RAISE EXCEPTION 'Duplicate app relation' USING ERRCODE='23505'; END IF;
   pos:=0;
   FOR target IN SELECT jsonb_array_elements_text(v) LOOP
    pos:=pos+1;
    relation_bytes:=relation_bytes+octet_length(jsonb_build_object('tenant',NEW.tenant,'source_id',NEW.id,'target_id',target,'position',pos)::text);
    relation_rows:=relation_rows+1;
   END LOOP;
  ELSIF f->>'kind' NOT IN ('json','money','richtext') AND jsonb_typeof(v)<>'string' THEN
   RAISE EXCEPTION 'Text required' USING ERRCODE='23514';
  END IF;
  normalized:=normalized||jsonb_build_object(f->>'name',v);
 END LOOP;
 NEW.fields:=normalized;
 NEW.storage_rows:=1+relation_rows;
 NEW.storage_bytes:=octet_length((normalized||jsonb_build_object('tenant',NEW.tenant,'id',NEW.id,'revision',NEW.revision))::text)+relation_bytes;
 RETURN NEW;
END $_$;

CREATE FUNCTION public.app_record_project() RETURNS trigger
    LANGUAGE plpgsql SECURITY DEFINER
    SET search_path TO 'pg_catalog', 'public', 'pg_temp'
    AS $$
DECLARE f jsonb; v jsonb; target text; pos integer; shop text; package text; delta_rows bigint; delta_bytes bigint;
BEGIN
 shop:=CASE WHEN TG_OP='DELETE' THEN OLD.tenant ELSE NEW.tenant END;
 package:=CASE WHEN TG_OP='DELETE' THEN OLD.app ELSE NEW.app END;
 delta_rows:=(CASE WHEN TG_OP='DELETE' THEN 0 ELSE NEW.storage_rows END)-(CASE WHEN TG_OP='INSERT' THEN 0 ELSE OLD.storage_rows END);
 delta_bytes:=(CASE WHEN TG_OP='DELETE' THEN 0 ELSE NEW.storage_bytes END)-(CASE WHEN TG_OP='INSERT' THEN 0 ELSE OLD.storage_bytes END);
 UPDATE app_storage_usage SET rows=rows+delta_rows,bytes=bytes+delta_bytes,updated_at=now() WHERE tenant=shop AND app=package;
 IF NOT FOUND THEN RAISE EXCEPTION 'App storage quota is not initialized' USING ERRCODE='42501'; END IF;
 IF TG_OP='DELETE' THEN RETURN OLD; END IF;
 DELETE FROM app_record_indexes WHERE tenant=NEW.tenant AND app=NEW.app AND entity=NEW.entity AND record_id=NEW.id;
 DELETE FROM app_record_references WHERE tenant=NEW.tenant AND app=NEW.app AND entity=NEW.entity AND record_id=NEW.id;
 FOR f IN SELECT value FROM app_models m,jsonb_array_elements(m.definition->'fields') WHERE m.tenant=NEW.tenant AND m.app=NEW.app AND m.entity=NEW.entity LOOP
  v:=NEW.fields->(f->>'name');
  IF v='null'::jsonb OR v IS NULL THEN CONTINUE; END IF;
  IF coalesce((f->>'indexed')::boolean,false) OR coalesce((f->>'unique')::boolean,false) THEN
   INSERT INTO app_record_indexes(tenant,app,entity,field,record_id,value,is_unique) VALUES(NEW.tenant,NEW.app,NEW.entity,f->>'name',NEW.id,v,coalesce((f->>'unique')::boolean,false));
  END IF;
  IF f->>'kind'='relations' THEN
   pos:=0;
   FOR target IN SELECT jsonb_array_elements_text(v) LOOP
    pos:=pos+1;
    INSERT INTO app_record_references(tenant,app,entity,record_id,field,position,target_entity,target_id) VALUES(NEW.tenant,NEW.app,NEW.entity,NEW.id,f->>'name',pos,f->>'references',target);
   END LOOP;
  ELSIF f->>'references' IS NOT NULL THEN
   INSERT INTO app_record_references(tenant,app,entity,record_id,field,position,target_entity,target_id) VALUES(NEW.tenant,NEW.app,NEW.entity,NEW.id,f->>'name',1,f->>'references',v#>>'{}');
  ELSIF f->>'coreReference' IS NOT NULL OR f->>'kind' IN ('image','file') THEN
   target:=CASE WHEN f->>'kind' IN ('image','file') THEN 'asset' ELSE f->>'coreReference' END;
   INSERT INTO app_record_references(tenant,app,entity,record_id,field,position,product_id,customer_id,order_id,channel_id,company_id,unit_id,asset_id)
    VALUES(NEW.tenant,NEW.app,NEW.entity,NEW.id,f->>'name',1,
     CASE WHEN target='product' THEN v#>>'{}' END,CASE WHEN target='customer' THEN v#>>'{}' END,
     CASE WHEN target='order' THEN v#>>'{}' END,CASE WHEN target='salesChannel' THEN v#>>'{}' END,
     CASE WHEN target='b2bCompany' THEN v#>>'{}' END,CASE WHEN target='b2bUnit' THEN v#>>'{}' END,
     CASE WHEN target='asset' THEN v#>>'{}' END);
  END IF;
 END LOOP;
 RETURN NEW;
END $$;

CREATE FUNCTION public.app_storage_accounting() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
DECLARE shop text; row_delta bigint; byte_delta bigint; expected_table text;
BEGIN
 shop := CASE WHEN TG_OP='DELETE' THEN OLD.tenant ELSE NEW.tenant END;
 expected_table := 'app_'||substr(encode(sha256(convert_to(shop||':'||TG_ARGV[0],'UTF8')),'hex'),1,24)||'_'||TG_ARGV[1];
 IF TG_TABLE_NAME<>expected_table OR (TG_OP='UPDATE' AND NEW.tenant<>OLD.tenant) THEN
   RAISE EXCEPTION 'App storage tenant/table mismatch' USING ERRCODE='42501';
 END IF;
 row_delta := CASE WHEN TG_OP='INSERT' THEN 1 WHEN TG_OP='DELETE' THEN -1 ELSE 0 END;
 byte_delta := CASE WHEN TG_OP='DELETE' THEN 0 ELSE octet_length(to_jsonb(NEW)::text) END
             - CASE WHEN TG_OP='INSERT' THEN 0 ELSE octet_length(to_jsonb(OLD)::text) END;
 -- UPDATE serializes concurrent writes against this app's quota, independent of other tenants.
 UPDATE app_storage_usage SET rows=rows+row_delta,bytes=bytes+byte_delta,updated_at=now()
 WHERE tenant=shop AND app=TG_ARGV[0];
 IF NOT FOUND THEN RAISE EXCEPTION 'App storage quota is not initialized' USING ERRCODE='42501'; END IF;
 RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END $$;

CREATE FUNCTION public.app_sync_relations() RETURNS trigger
    LANGUAGE plpgsql
    AS $_$
DECLARE links text; values_json jsonb; item text; ordinal bigint;
BEGIN
 links := TG_ARGV[0]; values_json := to_jsonb(NEW)->TG_ARGV[1];
 IF TG_TABLE_NAME<>TG_ARGV[2] OR NEW.tenant<>current_setting('rac.tenant',true) THEN
   RAISE EXCEPTION 'App relation tenant/table mismatch' USING ERRCODE='42501';
 END IF;
 EXECUTE format('DELETE FROM public.%I WHERE tenant=$1 AND source_id=$2',links) USING NEW.tenant,NEW.id;
 IF values_json IS NOT NULL AND values_json<>'null'::jsonb THEN
   IF jsonb_typeof(values_json)<>'array' OR jsonb_array_length(values_json)>100 THEN
     RAISE EXCEPTION 'App relations require at most 100 record IDs' USING ERRCODE='23514';
   END IF;
   FOR item,ordinal IN SELECT value, ordinality FROM jsonb_array_elements_text(values_json) WITH ORDINALITY LOOP
     EXECUTE format('INSERT INTO public.%I(tenant,source_id,target_id,position) VALUES($1,$2,$3,$4)',links)
       USING NEW.tenant,NEW.id,item,ordinal;
   END LOOP;
 END IF;
 RETURN NEW;
END $_$;

CREATE FUNCTION public.capture_entity_history() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
DECLARE rowdata jsonb;t text;k text;s jsonb;kind text;
BEGIN
 rowdata:=CASE WHEN TG_OP='DELETE' THEN to_jsonb(OLD) ELSE to_jsonb(NEW) END;
 t:=rowdata->>'tenant';kind:=TG_ARGV[0];k:=CASE WHEN TG_ARGV[1]='tenant' THEN 'base' ELSE rowdata->>TG_ARGV[1] END;
 IF TG_WHEN='BEFORE' THEN
  IF NOT EXISTS(SELECT 1 FROM entity_history WHERE tenant=t AND entity=kind AND entity_id=k AND transaction_id=txid_current()) THEN
   s:=entity_history_snapshot(kind,t,k);
   INSERT INTO entity_history(tenant,entity,entity_id,transaction_id,actor,source,reason,before_state)
   VALUES(t,kind,k,txid_current(),nullif(current_setting('veyndo.actor',true),''),coalesce(nullif(current_setting('veyndo.source',true),''),'system'),nullif(current_setting('veyndo.reason',true),''),s);
  END IF;
 ELSIF EXISTS(SELECT 1 FROM entity_history WHERE tenant=t AND entity=kind AND entity_id=k AND transaction_id=txid_current() AND NOT finalized) THEN
  s:=entity_history_snapshot(kind,t,k);
  UPDATE entity_history SET after_state=s,finalized=true WHERE tenant=t AND entity=kind AND entity_id=k AND transaction_id=txid_current();
 END IF;
 RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END $$;

CREATE FUNCTION public.claim_resource_lease(p_id text, p_tenant text, p_class text, p_limit bigint, p_slots integer) RETURNS boolean
    LANGUAGE plpgsql
    SET search_path TO 'public', 'pg_temp'
    AS $$
BEGIN
 IF p_slots<1 OR p_limit<1 THEN RAISE EXCEPTION 'Invalid resource budget'; END IF;
 PERFORM pg_advisory_xact_lock(hashtextextended('resource:'||p_tenant||':'||p_class,817361));
 DELETE FROM resource_leases WHERE tenant=p_tenant AND class=p_class AND expires_at<=now();
 IF (SELECT coalesce(sum(slots),0) FROM resource_leases WHERE tenant=p_tenant AND class=p_class)+p_slots>p_limit THEN RETURN false; END IF;
 INSERT INTO resource_leases(id,tenant,class,expires_at,slots) VALUES(p_id,p_tenant,p_class,now()+interval '310 seconds',p_slots);
 RETURN true;
END $$;

CREATE FUNCTION public.cognitive_error_delta() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
DECLARE old_code text; new_code text; old_shop text; new_shop text;
BEGIN
 IF TG_OP<>'INSERT' THEN old_code=OLD.error_code; old_shop=OLD.tenant; END IF;
 IF TG_OP<>'DELETE' THEN new_code=NEW.error_code; new_shop=NEW.tenant; END IF;
 IF old_code IS NOT DISTINCT FROM new_code AND old_shop IS NOT DISTINCT FROM new_shop THEN RETURN NULL; END IF;
 IF old_code IS NOT NULL THEN
  UPDATE knowledge_index_status SET errors=CASE
   WHEN coalesce((errors->>old_code)::bigint,0)<=1 THEN errors-old_code
   ELSE jsonb_set(errors,ARRAY[old_code],to_jsonb((errors->>old_code)::bigint-1)) END
  WHERE tenant=old_shop;
 END IF;
 IF new_code IS NOT NULL AND EXISTS(SELECT 1 FROM tenants WHERE id=new_shop) THEN
  INSERT INTO knowledge_index_status(tenant) VALUES(new_shop) ON CONFLICT DO NOTHING;
  UPDATE knowledge_index_status SET errors=jsonb_set(errors,ARRAY[new_code],to_jsonb(coalesce((errors->>new_code)::bigint,0)+1))
  WHERE tenant=new_shop;
 END IF;
 RETURN NULL;
END $$;

CREATE FUNCTION public.cognitive_relation_write() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
 IF TG_OP<>'INSERT' THEN
  INSERT INTO knowledge_relation_history VALUES(OLD.tenant,OLD.kind,OLD.source_id,OLD.target_id,OLD.revision,to_jsonb(OLD),OLD.recorded_from,clock_timestamp()) ON CONFLICT DO NOTHING;
 END IF;
 IF TG_OP='DELETE' THEN RETURN OLD; END IF;
 IF TG_OP='UPDATE' THEN NEW.revision=OLD.revision+1;NEW.recorded_from=clock_timestamp(); END IF;
 IF NEW.kind IN ('PAIRS_WITH','CO_PURCHASED') THEN NEW.target_kind='product';
 ELSIF NEW.kind='HAS_DOCUMENT' THEN NEW.target_kind='document';
 ELSIF NEW.kind='REFERENCES_PRODUCT' THEN NEW.source_kind='support';NEW.target_kind='product';
 END IF;
 IF NEW.kind IN ('SERVES','PAIRS_WITH') THEN NEW.state='confirmed'; END IF;
 INSERT INTO knowledge_nodes(tenant,kind,id) VALUES(NEW.tenant,NEW.source_kind,NEW.source_id),(NEW.tenant,NEW.target_kind,NEW.target_id) ON CONFLICT DO NOTHING;
 RETURN NEW;
END $$;

CREATE FUNCTION public.cognitive_source_changed() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
DECLARE object_key text;
BEGIN
 IF TG_TABLE_NAME='products' THEN
  object_key=NEW.id;
  IF TG_OP='UPDATE' AND NEW.name IS NOT DISTINCT FROM OLD.name AND NEW.description IS NOT DISTINCT FROM OLD.description AND NEW.category IS NOT DISTINCT FROM OLD.category THEN RETURN NEW; END IF;
 ELSE
  object_key=NEW.document_id||':'||NEW.position;
  IF TG_OP='UPDATE' AND NEW.text IS NOT DISTINCT FROM OLD.text THEN RETURN NEW; END IF;
 END IF;
 INSERT INTO embedding_jobs(tenant,kind,object_id) VALUES(NEW.tenant,TG_ARGV[0],object_key)
 ON CONFLICT(tenant,kind,object_id) DO UPDATE SET revision=embedding_jobs.revision+1,attempts=0,available_at=now(),lease=NULL,lease_until=NULL,error_code=NULL;
 RETURN NEW;
END $$;

CREATE FUNCTION public.cognitive_status_delta() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
DECLARE shop text; delta integer;
BEGIN
 shop=CASE WHEN TG_OP='DELETE' THEN OLD.tenant ELSE NEW.tenant END;
 delta=CASE WHEN TG_OP='DELETE' THEN -1 ELSE 1 END;
 IF EXISTS(SELECT 1 FROM tenants WHERE id=shop) THEN
  INSERT INTO knowledge_index_status(tenant) VALUES(shop) ON CONFLICT DO NOTHING;
  UPDATE knowledge_index_status SET
   indexed_products=indexed_products+CASE WHEN TG_TABLE_NAME='semantic_products' THEN delta ELSE 0 END,
   pending_jobs=pending_jobs+CASE WHEN TG_TABLE_NAME='embedding_jobs' THEN delta ELSE 0 END
  WHERE tenant=shop;
 END IF;
 RETURN NULL;
END $$;

CREATE FUNCTION public.cognitive_vector_digest() RETURNS trigger
    LANGUAGE plpgsql
    AS $$ BEGIN NEW.vector_digest=md5(NEW.embedding::text); RETURN NEW; END $$;

CREATE FUNCTION public.emit_agent_run_state() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
 IF NEW.run_state IS NOT NULL AND (TG_OP='INSERT' OR NEW.run_state IS DISTINCT FROM OLD.run_state) THEN
  INSERT INTO outbox(tenant,kind,data) VALUES(NEW.tenant,'intelligence.run.changed',
   jsonb_build_object('taskId',NEW.id,'state',NEW.run_state,'revision',NEW.run_revision));
 END IF;
 RETURN NEW;
END $$;

CREATE FUNCTION public.emit_read_cache_event() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
 IF TG_OP='DELETE' THEN RETURN OLD; END IF;
 INSERT INTO outbox(tenant,kind,data) VALUES(NEW.tenant,'cache.invalidate',jsonb_build_object('namespace',TG_TABLE_NAME));
 RETURN NEW;
END $$;

CREATE FUNCTION public.entity_history_snapshot(kind text, t text, k text) RETURNS jsonb
    LANGUAGE plpgsql
    AS $$
DECLARE s jsonb;
BEGIN
 IF kind='role' THEN SELECT to_jsonb(r)-'tenant' INTO s FROM workspace_roles r WHERE tenant=t AND id=k; RETURN s; END IF;
 RETURN entity_history_snapshot_before_roles(kind,t,k);
END $$;

CREATE FUNCTION public.entity_history_snapshot_before_b2b(kind text, t text, k text) RETURNS jsonb
    LANGUAGE plpgsql
    AS $$
DECLARE state jsonb;metadata jsonb;
BEGIN
 state:=entity_history_snapshot_v034(kind,t,k);
 IF kind='customer' AND state IS NOT NULL THEN
  SELECT jsonb_build_object('customerNumber',customer_number,'salesChannelId',sales_channel_id,'languageId',language_id,'firstLogin',first_login,'lastLogin',last_login,'lastPaymentMethodId',last_payment_method_id,'automation',automation) INTO metadata FROM customers WHERE tenant=t AND email=k;
  state:=state||metadata;
 END IF;
 RETURN state;
END $$;

CREATE FUNCTION public.entity_history_snapshot_before_roles(kind text, t text, k text) RETURNS jsonb
    LANGUAGE plpgsql
    AS $$
DECLARE s jsonb;
BEGIN
 IF kind='b2bCompany' THEN
  SELECT to_jsonb(r)-'tenant' INTO s FROM b2b_companies r WHERE tenant=t AND id=k;
 ELSIF kind='b2bUnit' THEN
  SELECT (to_jsonb(r)-'tenant')||jsonb_build_object(
   'members',coalesce((SELECT jsonb_agg(to_jsonb(m)-'tenant' ORDER BY customer_id) FROM b2b_members m WHERE m.tenant=t AND m.unit_id=k),'[]'),
   'catalog',coalesce((SELECT jsonb_agg(product_id ORDER BY product_id) FROM b2b_catalog WHERE tenant=t AND unit_id=k),'[]'))
   INTO s FROM b2b_units r WHERE tenant=t AND id=k;
 ELSIF kind='b2bQuote' THEN
  SELECT (to_jsonb(r)-'tenant') #- '{data,quote,token}' INTO s FROM b2b_quotes r WHERE tenant=t AND id=k;
 ELSIF kind='b2bList' THEN
  SELECT to_jsonb(r)-'tenant' INTO s FROM b2b_lists r WHERE tenant=t AND id=k;
 ELSIF kind='b2bApproval' THEN
  SELECT (to_jsonb(r)-'tenant') #- '{data,quote,token}' INTO s FROM b2b_approvals r WHERE tenant=t AND id=k;
 ELSE RETURN entity_history_snapshot_before_b2b(kind,t,k);
 END IF;
 RETURN s;
END $$;

CREATE FUNCTION public.entity_history_snapshot_v034(kind text, t text, k text) RETURNS jsonb
    LANGUAGE plpgsql
    AS $_$
DECLARE state jsonb;tab text;keycol text;
BEGIN
 IF kind='product' THEN
  SELECT jsonb_build_object('record',to_jsonb(p)-'tenant',
   'translations',coalesce((SELECT jsonb_object_agg(l.locale,jsonb_build_object('name',x.name,'description',x.description)) FROM product_translations x JOIN languages l ON l.id=x.language_id WHERE x.tenant=t AND x.product_id=k),'{}'),
   'categories',coalesce((SELECT jsonb_agg(category_id ORDER BY category_id) FROM product_categories WHERE tenant=t AND product_id=k),'[]'),
   'channels',coalesce((SELECT jsonb_agg(jsonb_build_object('id',channel_id,'visible',visible) ORDER BY channel_id) FROM product_channel_visibility WHERE tenant=t AND product_id=k),'[]')) INTO state FROM products p WHERE p.tenant=t AND p.id=k;
  RETURN state;
 ELSIF kind='customer' THEN
  SELECT jsonb_build_object('id',c.id,'email',c.email,'profile',c.profile,'company',c.company,'customerGroup',c.group_name,'active',c.active,'revision',c.revision,'defaultBillingAddressId',c.default_billing_address_id,'defaultShippingAddressId',c.default_shipping_address_id,
   'addresses',coalesce((SELECT jsonb_agg(jsonb_build_object('id',a.id,'address',a.data,'revision',a.revision) ORDER BY a.id) FROM customer_addresses a WHERE a.tenant=t AND a.email=k),'[]')) INTO state FROM customers c WHERE c.tenant=t AND c.email=k;
  RETURN state;
 ELSIF kind='order' THEN
  SELECT data #- '{cart,token}' INTO state FROM orders WHERE tenant=t AND id=k;RETURN state;
 END IF;
 SELECT x.tab,x.keycol INTO tab,keycol FROM (VALUES
  ('category','categories','id'),('settings','commerce_settings','tenant'),('checkoutChannel','commerce_overrides','channel_id'),
  ('company','receipt_settings','tenant'),('companyChannel','company_overrides','channel_id'),
  ('rule','commerce_rules','id'),('flow','commerce_flows','id'),('promotion','commerce_promotions','id'),
  ('channel','sales_channels','id'),('source','knowledge_documents','id')) x(kind,tab,keycol) WHERE x.kind=$1;
 IF tab IS NULL THEN RAISE EXCEPTION 'Unknown history entity'; END IF;
 EXECUTE format('SELECT to_jsonb(r)-''tenant'' FROM %I r WHERE tenant=$1 AND %I=$2',tab,keycol) INTO state USING t,CASE WHEN keycol='tenant' THEN t ELSE k END;
 RETURN state;
END $_$;

CREATE FUNCTION public.initialize_workspace_roles() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN PERFORM seed_workspace_roles(NEW.id); RETURN NEW; END $$;

CREATE FUNCTION public.intelligence_capture_clock() RETURNS trigger
    LANGUAGE plpgsql
    AS $$BEGIN
 IF NEW.environment='live' AND NEW.state IN ('captured','captured_late') AND NEW.capture_id IS NOT NULL
 AND NEW.capture_confirmed_at IS NULL THEN NEW.capture_confirmed_at=now();END IF;
 IF TG_OP='UPDATE' AND OLD.capture_confirmed_at IS NOT NULL THEN NEW.capture_confirmed_at=OLD.capture_confirmed_at;END IF;
 RETURN NEW;END$$;

CREATE FUNCTION public.knowledge_chunk_words() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
 IF TG_OP='UPDATE' AND OLD.text=NEW.text THEN RETURN NULL; END IF;
 DELETE FROM knowledge_chunk_lexemes WHERE tenant=NEW.tenant AND document_id=NEW.document_id AND position=NEW.position;
 INSERT INTO knowledge_chunk_lexemes SELECT NEW.tenant,NEW.document_id,NEW.position,word
 FROM unnest(tsvector_to_array(to_tsvector('simple',NEW.text))) word;
 RETURN NULL;
END $$;

CREATE FUNCTION public.knowledge_product_words() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
 IF TG_OP='UPDATE' AND OLD.name=NEW.name AND OLD.description=NEW.description THEN RETURN NULL; END IF;
 DELETE FROM knowledge_product_lexemes WHERE tenant=NEW.tenant AND product_id=NEW.id;
 INSERT INTO knowledge_product_lexemes SELECT NEW.tenant,NEW.id,word
 FROM unnest(tsvector_to_array(to_tsvector('simple',NEW.name||' '||NEW.description))) word;
 RETURN NULL;
END $$;

CREATE FUNCTION public.notify_outbox_cache() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
 PERFORM pg_notify('veyndo_read_invalidation',json_build_object('tenant',NEW.tenant)::text);
 RETURN NEW;
END $$;

CREATE FUNCTION public.notify_work_ready() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
 IF TG_OP='INSERT' OR coalesce(to_jsonb(NEW)->>'state',to_jsonb(NEW)->>'status')='queued' OR TG_TABLE_NAME='outbox' AND to_jsonb(NEW)->>'delivered_at' IS NULL THEN
  PERFORM pg_notify('veyndo_work_ready',TG_TABLE_NAME);
 END IF;
 RETURN NEW;
END $$;

CREATE FUNCTION public.order_document_accounting() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
DECLARE shop text; target_app text; delta_rows bigint; delta_bytes bigint;
BEGIN
 shop := CASE WHEN TG_OP='DELETE' THEN OLD.tenant ELSE NEW.tenant END;
 target_app := CASE WHEN TG_OP='DELETE' THEN OLD.source_app ELSE NEW.source_app END;
 IF target_app IS NULL THEN RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END; END IF;
 IF TG_OP='UPDATE' THEN RAISE EXCEPTION 'Imported order documents are immutable' USING ERRCODE='42501'; END IF;
 delta_rows := CASE WHEN TG_OP='INSERT' THEN 1 ELSE -1 END;
 delta_bytes := CASE WHEN TG_OP='INSERT' THEN octet_length(NEW.content)+octet_length(NEW.snapshot::text)+512 ELSE -(octet_length(OLD.content)+octet_length(OLD.snapshot::text)+512) END;
 UPDATE app_storage_usage SET rows=rows+delta_rows,bytes=bytes+delta_bytes,updated_at=now() WHERE tenant=shop AND app=target_app;
 IF NOT FOUND THEN RAISE EXCEPTION 'App storage quota is not initialized' USING ERRCODE='42501'; END IF;
 RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END $$;

CREATE FUNCTION public.prepare_outbox_delivery() RETURNS trigger
    LANGUAGE plpgsql
    SET search_path TO 'pg_catalog', 'public'
    AS $$
BEGIN
 NEW.dispatch_not_before=NEW.available_at;
 NEW.dispatch_requires_retry=NEW.dead_letter_at IS NOT NULL;
 RETURN NEW;
END $$;

CREATE FUNCTION public.product_content_clock() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
 IF NEW.revision IS DISTINCT FROM OLD.revision THEN
  IF NEW.stock IS NOT DISTINCT FROM OLD.stock OR
     (to_jsonb(NEW)-ARRAY['stock','revision','content_revision']) IS DISTINCT FROM
     (to_jsonb(OLD)-ARRAY['stock','revision','content_revision']) THEN
   NEW.content_revision:=OLD.content_revision+1;
  ELSE
   NEW.content_revision:=OLD.content_revision;
  END IF;
 END IF;
 RETURN NEW;
END;
$$;

CREATE FUNCTION public.queue_vector_index() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
DECLARE row_data jsonb;
BEGIN
 row_data := CASE WHEN TG_OP='DELETE' THEN to_jsonb(OLD) ELSE to_jsonb(NEW) END;
 INSERT INTO vector_index_queue(tenant,kind,object_id) VALUES(row_data->>'tenant',TG_ARGV[0],CASE WHEN TG_ARGV[0]='product' THEN row_data->>'product_id' ELSE (row_data->>'document_id')||':'||(row_data->>'position') END)
 ON CONFLICT(tenant,kind,object_id) DO UPDATE SET updated_at=now(),revision=vector_index_queue.revision+1,lease=NULL,lease_until=NULL,attempts=0,error_code=NULL,available_at=now();
 RETURN NEW;
END $$;

CREATE FUNCTION public.register_outbox_subscribers() RETURNS trigger
    LANGUAGE plpgsql
    SET search_path TO 'pg_catalog', 'public'
    AS $$
BEGIN
 INSERT INTO public.outbox_checkpoints(tenant,subscriber)
  SELECT NEW.id,s FROM unnest(ARRAY['cognition','apps','flows','audit']) s;
 RETURN NEW;
END $$;

CREATE FUNCTION public.register_outbox_tenant() RETURNS trigger
    LANGUAGE plpgsql
    SET search_path TO 'pg_catalog', 'public'
    AS $$
BEGIN
 INSERT INTO public.outbox_tenant_dispatch(tenant) VALUES(NEW.tenant) ON CONFLICT DO NOTHING;
 RETURN NEW;
END $$;

CREATE FUNCTION public.seed_shop_automation(shop text) RETURNS void
    LANGUAGE plpgsql
    AS $$
DECLARE
 settings jsonb; owner_id text; languages jsonb; main_language text;
 names jsonb; condition jsonb; flow jsonb;
BEGIN
 SELECT data INTO settings FROM commerce_settings WHERE tenant=shop;
 IF settings IS NULL THEN RETURN; END IF;
 languages := coalesce(settings->'locales','["en-GB","de-DE","fr-FR","es-ES"]'::jsonb); main_language := coalesce(settings->>'mainLocale','en-GB');
 SELECT user_id INTO owner_id FROM memberships WHERE tenant=shop AND role='owner' AND active ORDER BY user_id LIMIT 1;
 owner_id := coalesce(owner_id,'bootstrap');
 names := '{"en-GB":"Main storefront","de-DE":"Haupt-Verkaufskanal","es-ES":"Tienda principal","fr-FR":"Boutique principale"}';
 IF NOT names ? main_language THEN names := names || jsonb_build_object(main_language,'Main storefront'); END IF;
 INSERT INTO sales_channels(tenant,id,data) VALUES(shop,'default',jsonb_build_object('name',names,'kind','storefront','active',true,'locales',languages,'productIds','[]'::jsonb,'navigationCategoryId',NULL)) ON CONFLICT DO NOTHING;
 FOR names,condition IN SELECT * FROM (VALUES
 ('{"en-GB":"All orders","de-DE":"Alle Bestellungen","es-ES":"Todos los pedidos","fr-FR":"Toutes les commandes"}'::jsonb,'{"type":"alwaysValid"}'::jsonb),
 ('{"en-GB":"Business customers","de-DE":"Geschäftskunden","es-ES":"Clientes empresariales","fr-FR":"Clients professionnels"}'::jsonb,'{"type":"customerGroup","operator":"=","values":["business"]}'::jsonb),
 ('{"en-GB":"Cart from 100","de-DE":"Warenkorb ab 100","es-ES":"Carrito desde 100","fr-FR":"Panier à partir de 100"}'::jsonb,'{"type":"cartCartAmount","operator":">=","amount":100}'::jsonb)
 ) AS templates(n,c) LOOP
  IF NOT names ? main_language THEN names := names || jsonb_build_object(main_language,names->>'en-GB'); END IF;
  INSERT INTO commerce_rules(tenant,id,name,condition) VALUES(shop,
   CASE condition->>'type' WHEN 'alwaysValid' THEN 'default_all_orders' WHEN 'customerGroup' THEN 'default_business_customers' ELSE 'default_cart_100' END,names,condition) ON CONFLICT DO NOTHING;
 END LOOP;
 FOR flow IN SELECT value FROM jsonb_array_elements('[
 {"id":"default_order_received","event":"order.placed","name":{"en-GB":"Order received","de-DE":"Bestellung eingegangen","es-ES":"Pedido recibido","fr-FR":"Commande reçue"},"instruction":{"en-GB":"Order received. Check fulfillment and payment before dispatch.","de-DE":"Bestellung eingegangen. Lieferung und Zahlung vor dem Versand prüfen.","es-ES":"Pedido recibido. Revisa la entrega y el pago antes del envío.","fr-FR":"Commande reçue. Vérifiez la livraison et le paiement avant expédition."}},
 {"id":"default_payment_received","event":"payment.captured","name":{"en-GB":"Payment received","de-DE":"Zahlung eingegangen","es-ES":"Pago recibido","fr-FR":"Paiement reçu"},"instruction":{"en-GB":"Payment capture confirmed by the payment event.","de-DE":"Zahlungseingang durch das Zahlungsereignis bestätigt.","es-ES":"Cobro confirmado por el evento de pago.","fr-FR":"Encaissement confirmé par l’événement de paiement."}}
 ]'::jsonb) LOOP
  IF NOT (flow->'name') ? main_language THEN
   flow := jsonb_set(flow,'{name}',flow->'name' || jsonb_build_object(main_language,flow->'name'->>'en-GB'));
   flow := jsonb_set(flow,'{instruction}',flow->'instruction' || jsonb_build_object(main_language,flow->'instruction'->>'en-GB'));
  END IF;
  INSERT INTO commerce_flows(tenant,id,data) VALUES(shop,flow->>'id',(flow-'id') || jsonb_build_object('active',true,'condition',jsonb_build_object('type','ruleReference','ruleId','default_all_orders'),'action','note','locale',main_language,'actor',owner_id)) ON CONFLICT DO NOTHING;
 END LOOP;
END $$;

CREATE FUNCTION public.seed_workspace_roles(t text) RETURNS void
    LANGUAGE sql
    AS $$
 INSERT INTO workspace_roles(tenant,id,data,builtin) VALUES
(t,'owner',jsonb_build_object('name',jsonb_build_object('en-GB','Owner','de-DE','Inhaber','es-ES','Propietario','fr-FR','Propriétaire'),'permissions','["overview.read", "assistant.use", "automation.read", "automation.manage", "channels.read", "channels.manage", "developer.read", "developer.manage", "environments.read", "environments.manage", "knowledge.write", "ai.manage", "apps.read", "pos.use", "b2b.read", "b2b.manage", "catalog.read", "catalog.write", "orders.read", "orders.write", "customers.read", "customers.pii", "customers.write", "documents.read", "documents.create", "payments.read", "payments.manage", "team.manage", "settings.read", "settings.write", "apps.manage", "knowledge.read"]'::jsonb,'active',true),true),
(t,'admin',jsonb_build_object('name',jsonb_build_object('en-GB','Admin','de-DE','Administrator','es-ES','Administrador','fr-FR','Administrateur'),'permissions','["overview.read", "assistant.use", "automation.read", "automation.manage", "channels.read", "channels.manage", "developer.read", "developer.manage", "environments.read", "environments.manage", "knowledge.write", "ai.manage", "apps.read", "pos.use", "b2b.read", "b2b.manage", "catalog.read", "catalog.write", "orders.read", "orders.write", "customers.read", "customers.pii", "customers.write", "documents.read", "documents.create", "payments.read", "payments.manage", "team.manage", "settings.read", "settings.write", "apps.manage", "knowledge.read"]'::jsonb,'active',true),true),
(t,'editor',jsonb_build_object('name',jsonb_build_object('en-GB','Editor','de-DE','Redakteur','es-ES','Editor','fr-FR','Éditeur'),'permissions','["catalog.read", "orders.read", "payments.read", "settings.read", "knowledge.read", "overview.read", "assistant.use", "automation.read", "channels.read", "environments.read", "apps.read", "catalog.write", "knowledge.write"]'::jsonb,'active',true),true),
(t,'viewer',jsonb_build_object('name',jsonb_build_object('en-GB','Viewer','de-DE','Leser','es-ES','Lector','fr-FR','Lecteur'),'permissions','["catalog.read", "orders.read", "payments.read", "settings.read", "knowledge.read", "overview.read", "assistant.use", "automation.read", "channels.read", "environments.read", "apps.read"]'::jsonb,'active',true),true) ON CONFLICT DO NOTHING;
$$;

CREATE FUNCTION public.stamp_language_registry() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    UPDATE read_context_versions SET version=gen_random_uuid() WHERE namespace='languages';
    RETURN NULL;
END;
$$;

CREATE FUNCTION public.stamp_new_languages() RETURNS trigger
    LANGUAGE plpgsql
    SET search_path TO 'pg_catalog', 'pg_temp'
    AS $$
BEGIN
 IF EXISTS(SELECT 1 FROM changed_languages) THEN
  UPDATE public.read_context_versions SET version=gen_random_uuid() WHERE namespace='languages';
 END IF;
 RETURN NULL;
END;
$$;

CREATE FUNCTION public.stamp_read_context() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.cache_version := gen_random_uuid();
    RETURN NEW;
END;
$$;

CREATE FUNCTION public.veyndo_activate_scope() RETURNS void
    LANGUAGE plpgsql SECURITY DEFINER
    SET search_path TO 'pg_catalog', 'pg_temp'
    AS $$
DECLARE c veyndo_authority.configuration;
BEGIN
 SELECT * INTO STRICT c FROM veyndo_authority.configuration FOR UPDATE;
 IF session_user IS DISTINCT FROM c.migration_role OR c.tenant_login IS NULL THEN
  RAISE EXCEPTION 'Activation requires configured roles and the migration connection' USING ERRCODE='42501';
 END IF;
 UPDATE veyndo_authority.configuration SET enforced=true;
END;
$$;

CREATE FUNCTION public.veyndo_authenticated_scope() RETURNS jsonb
    LANGUAGE plpgsql STABLE SECURITY DEFINER
    SET search_path TO 'pg_catalog', 'pg_temp'
    AS $_$
DECLARE c veyndo_authority.configuration; parts text[]; key_bytes bytea;
 expected bytea; supplied bytea; deadline bigint; epoch_now bigint; body text;
BEGIN
 SELECT * INTO STRICT c FROM veyndo_authority.configuration;
 IF session_user IS NOT DISTINCT FROM c.migration_role THEN
  RETURN jsonb_build_object('system',true,'migration',true,'authenticated',true);
 END IF;
 body:=current_setting('veyndo.scope_ticket',true);
 IF body IS NULL OR length(body)>512 THEN RETURN '{}'::jsonb; END IF;
 parts:=string_to_array(body,':');
 IF cardinality(parts)<>8 OR parts[1]<>'v1' OR parts[2]!~'^[a-f0-9]{32}$'
  OR parts[3]!~'^[0-9]{1,12}$' OR parts[4] NOT IN ('S','T','P','A','D')
  OR parts[5]!~'^[A-Za-z0-9_-]{0,64}$' OR parts[6]!~'^[A-Za-z0-9_-]{0,64}$'
  OR parts[7]!~'^[a-f0-9]{32}$' OR parts[8]!~'^[a-f0-9]{64}$' THEN RETURN '{}'::jsonb; END IF;
 deadline:=parts[3]::bigint; epoch_now:=floor(extract(epoch FROM statement_timestamp()))::bigint;
 IF deadline<epoch_now OR deadline>epoch_now+600 THEN RETURN '{}'::jsonb; END IF;
 SELECT k.secret INTO key_bytes FROM veyndo_authority.keys k
 WHERE k.id=parts[2] AND (k.active OR k.verify_until>statement_timestamp());
 IF key_bytes IS NULL THEN RETURN '{}'::jsonb; END IF;
 body:=array_to_string(parts[1:7],':')||':'||session_user;
 expected:=veyndo_authority.mac(convert_to(body,'UTF8'),key_bytes);
 supplied:=decode(parts[8],'hex');
 -- Blinded comparison does not expose the position of an incorrect MAC byte.
 IF veyndo_authority.mac(expected,key_bytes)<>veyndo_authority.mac(supplied,key_bytes) THEN RETURN '{}'::jsonb; END IF;
 IF parts[4]='S' THEN
  IF session_user IS DISTINCT FROM c.system_role OR parts[5]<>'' OR parts[6]<>'' THEN RETURN '{}'::jsonb; END IF;
  RETURN jsonb_build_object('system',true,'authenticated',true);
 END IF;
 IF session_user<>c.tenant_login OR session_user IS NOT DISTINCT FROM c.system_role THEN RETURN '{}'::jsonb; END IF;
 IF parts[4]='D' THEN RETURN jsonb_build_object('system',false,'authenticated',true); END IF;
 IF parts[5]='' OR parts[4] IN ('A','P') AND parts[6]='' OR parts[4]='T' AND parts[6]<>'' THEN RETURN '{}'::jsonb; END IF;
 RETURN jsonb_build_object('system',false,'authenticated',true,'tenant',parts[5],
  'actor',nullif(parts[6],''),'personalActor',CASE WHEN parts[4]='P' THEN parts[6] ELSE NULL END);
END;
$_$;

CREATE FUNCTION public.veyndo_configure_scope(system_login name, tenant_login name) RETURNS void
    LANGUAGE plpgsql SECURITY DEFINER
    SET search_path TO 'pg_catalog', 'pg_temp'
    AS $$
DECLARE c veyndo_authority.configuration; r record;
BEGIN
 SELECT * INTO STRICT c FROM veyndo_authority.configuration FOR UPDATE;
 IF session_user IS DISTINCT FROM c.migration_role THEN
  RAISE EXCEPTION 'Scope configuration requires the migration connection' USING ERRCODE='42501';
 END IF;
 IF system_login=tenant_login OR system_login=session_user OR tenant_login=session_user
  OR NOT EXISTS(SELECT 1 FROM pg_roles WHERE rolname=system_login AND rolcanlogin AND NOT rolsuper AND NOT rolbypassrls AND NOT rolcreaterole)
  OR NOT EXISTS(SELECT 1 FROM pg_roles WHERE rolname=tenant_login AND rolcanlogin AND NOT rolsuper AND NOT rolbypassrls AND NOT rolcreaterole AND NOT rolcreatedb AND NOT rolreplication)
  OR EXISTS(SELECT 1 FROM pg_roles WHERE rolname<>tenant_login AND pg_has_role(tenant_login,oid,'MEMBER'))
  OR EXISTS(SELECT 1 FROM pg_class WHERE relowner=(SELECT oid FROM pg_roles WHERE rolname=tenant_login)) THEN
  RAISE EXCEPTION 'Authority roles must be distinct restricted logins; tenant login cannot inherit roles or own relations' USING ERRCODE='42501';
 END IF;
 IF c.enforced AND c.tenant_login IS DISTINCT FROM tenant_login THEN
  RAISE EXCEPTION 'Use a coordinated credential cutover before replacing an active tenant login';
 END IF;
 EXECUTE format('GRANT %I TO %I',system_login,session_user);
 EXECUTE format('REVOKE ALL ON SCHEMA public FROM %I',tenant_login);
 EXECUTE format('GRANT USAGE ON SCHEMA public TO %I',tenant_login);
 EXECUTE format('GRANT CONNECT ON DATABASE %I TO %I',current_database(),tenant_login);
 -- PUBLIC's default TEMP grant would let one tenant session shadow native table
 -- names on a reused backend. Revoking a login's own grant cannot remove PUBLIC.
 EXECUTE format('REVOKE TEMPORARY ON DATABASE %I FROM PUBLIC',current_database());
 EXECUTE format('GRANT TEMPORARY ON DATABASE %I TO %I,%I',current_database(),system_login,session_user);
 EXECUTE format('REVOKE CREATE,TEMPORARY ON DATABASE %I FROM %I',current_database(),tenant_login);
 FOR r IN SELECT t.relname FROM pg_class t JOIN pg_namespace n ON n.oid=t.relnamespace
  WHERE n.nspname='public' AND t.relkind IN ('r','v','m','S') LOOP
  EXECUTE format('REVOKE ALL ON public.%I FROM %I',r.relname,tenant_login);
 END LOOP;
 FOR r IN SELECT t.relname FROM pg_class t JOIN pg_namespace n ON n.oid=t.relnamespace
  WHERE n.nspname='public' AND t.relkind='r' AND t.relrowsecurity AND t.relforcerowsecurity
   AND t.relname NOT LIKE 'connector_%'
   AND EXISTS(SELECT 1 FROM pg_attribute a WHERE a.attrelid=t.oid AND a.attname='tenant' AND NOT a.attisdropped) LOOP
  EXECUTE format('GRANT SELECT,INSERT,UPDATE,DELETE,REFERENCES ON public.%I TO %I',r.relname,tenant_login);
 END LOOP;
 EXECUTE format('GRANT SELECT,UPDATE ON public.tenants TO %I',tenant_login);
 EXECUTE format('GRANT SELECT(id,name,email) ON public.merchant_users TO %I',tenant_login);
 EXECUTE format('GRANT SELECT ON public.membership_grants,public.languages,public.read_context_versions,public.commerce_migrations TO %I',tenant_login);
 FOR r IN SELECT t.relname FROM pg_class t JOIN pg_namespace n ON n.oid=t.relnamespace
  WHERE n.nspname='public' AND t.relkind='S' AND t.relname NOT LIKE 'connector_%' LOOP
  EXECUTE format('GRANT USAGE,SELECT ON public.%I TO %I',r.relname,tenant_login);
 END LOOP;
 UPDATE veyndo_authority.configuration SET system_role=system_login,tenant_login=veyndo_configure_scope.tenant_login;
END;
$$;

CREATE FUNCTION public.veyndo_guard_app_table(table_name text) RETURNS void
    LANGUAGE plpgsql SECURITY DEFINER
    SET search_path TO 'pg_catalog', 'pg_temp'
    AS $$
DECLARE c veyndo_authority.configuration; target oid;
BEGIN
 SELECT * INTO STRICT c FROM veyndo_authority.configuration;
 target:=to_regclass(format('public.%I',table_name));
 IF session_user IS DISTINCT FROM c.system_role AND session_user IS DISTINCT FROM c.migration_role
  AND NOT (NOT c.enforced AND EXISTS(SELECT 1 FROM pg_class WHERE oid=target AND pg_has_role(session_user,relowner,'MEMBER'))) THEN
  RAISE EXCEPTION 'App schema changes require the system connection' USING ERRCODE='42501';
 END IF;
 IF table_name!~'^app_[a-f0-9]{16}([a-f0-9]{8})?_' OR target IS NULL
  OR NOT EXISTS(SELECT 1 FROM pg_class WHERE oid=target AND relrowsecurity AND relforcerowsecurity)
  OR NOT EXISTS(SELECT 1 FROM pg_attribute WHERE attrelid=target AND attname='tenant' AND NOT attisdropped) THEN
  RAISE EXCEPTION 'Invalid protected app table' USING ERRCODE='42501';
 END IF;
 IF NOT EXISTS(SELECT 1 FROM pg_policy WHERE polrelid=target AND polname='authenticated_scope') THEN
  EXECUTE format('CREATE POLICY authenticated_scope ON public.%I AS RESTRICTIVE USING (coalesce((SELECT (public.veyndo_scope_guard()->>''system'')::boolean),false) OR tenant=(SELECT public.veyndo_scope_guard()->>''tenant'')) WITH CHECK (coalesce((SELECT (public.veyndo_scope_guard()->>''system'')::boolean),false) OR tenant=(SELECT public.veyndo_scope_guard()->>''tenant''))',table_name);
 END IF;
 IF c.tenant_login IS NOT NULL THEN
  EXECUTE format('GRANT SELECT,INSERT,UPDATE,DELETE,REFERENCES ON public.%I TO %I',table_name,c.tenant_login);
 END IF;
END;
$$;

CREATE FUNCTION public.veyndo_merge_http_counts(a jsonb, b jsonb) RETURNS jsonb
    LANGUAGE sql IMMUTABLE PARALLEL SAFE
    AS $_$
 SELECT coalesce(jsonb_object_agg(key,n),'{}'::jsonb)
 FROM (SELECT key,sum(value::bigint) AS n
       FROM (SELECT * FROM jsonb_each_text(a) UNION ALL SELECT * FROM jsonb_each_text(b)) v
       WHERE key ~ '^[45][0-9]{2}$' GROUP BY key) totals
$_$;

CREATE FUNCTION public.veyndo_register_language(new_locale text) RETURNS void
    LANGUAGE plpgsql SECURITY DEFINER
    SET search_path TO 'pg_catalog', 'pg_temp'
    AS $_$
DECLARE scope jsonb:=public.veyndo_scope_guard();
BEGIN
 IF NOT coalesce((scope->>'system')::boolean,false)
  AND NOT (coalesce((scope->>'authenticated')::boolean,false) AND scope->>'tenant' IS NOT NULL) THEN
  RAISE EXCEPTION 'Language registration requires authenticated scope' USING ERRCODE='42501';
 END IF;
 IF length(new_locale)>35 OR new_locale!~'^[a-z]{2,3}(-[A-Z][a-z]{3})?(-([A-Z]{2}|[0-9]{3}))?(-([a-z0-9]{5,8}|[0-9][a-z0-9]{3}))?$' THEN
  RAISE EXCEPTION 'Invalid canonical language';
 END IF;
 IF EXISTS(SELECT 1 FROM public.languages l WHERE l.locale=new_locale) THEN RETURN; END IF;
 PERFORM pg_advisory_xact_lock(7193512);
 IF (SELECT count(*) FROM public.languages)>=10000 THEN RAISE EXCEPTION 'Language registry capacity reached'; END IF;
 INSERT INTO public.languages(id,locale)
  VALUES(substr(encode(sha256(convert_to('commerce-language:'||new_locale,'UTF8')),'hex'),1,32),new_locale)
  ON CONFLICT(locale) DO NOTHING;
END;
$_$;

CREATE FUNCTION public.veyndo_rotate_scope_key() RETURNS text
    LANGUAGE plpgsql SECURITY DEFINER
    SET search_path TO 'pg_catalog', 'pg_temp'
    AS $$
DECLARE c veyndo_authority.configuration; new_id text:=replace(gen_random_uuid()::text,'-','');
BEGIN
 SELECT * INTO STRICT c FROM veyndo_authority.configuration FOR UPDATE;
 IF session_user IS DISTINCT FROM c.migration_role THEN
  RAISE EXCEPTION 'Scope rotation requires the migration connection' USING ERRCODE='42501';
 END IF;
 DELETE FROM veyndo_authority.keys WHERE NOT active AND verify_until<now();
 UPDATE veyndo_authority.keys SET active=false,verify_until=now()+interval '10 minutes' WHERE active;
 INSERT INTO veyndo_authority.keys(id,secret,active) VALUES(new_id,veyndo_authority.random_key(),true);
 RETURN new_id;
END;
$$;

CREATE FUNCTION public.veyndo_scope_guard() RETURNS jsonb
    LANGUAGE plpgsql STABLE SECURITY DEFINER
    SET search_path TO 'pg_catalog', 'pg_temp'
    AS $$
BEGIN
 IF NOT (SELECT enforced FROM veyndo_authority.configuration) THEN RETURN '{"system":true}'::jsonb; END IF;
 RETURN public.veyndo_authenticated_scope();
END;
$$;

CREATE FUNCTION public.veyndo_scope_key() RETURNS TABLE(id text, secret text, system_role name, tenant_login name)
    LANGUAGE plpgsql SECURITY DEFINER
    SET search_path TO 'pg_catalog', 'pg_temp'
    AS $$
DECLARE c veyndo_authority.configuration;
BEGIN
 SELECT * INTO STRICT c FROM veyndo_authority.configuration;
 IF session_user IS DISTINCT FROM c.system_role AND session_user IS DISTINCT FROM c.migration_role THEN
  RAISE EXCEPTION 'Database signing material requires the system connection' USING ERRCODE='42501';
 END IF;
 RETURN QUERY SELECT k.id,encode(k.secret,'hex'),c.system_role,c.tenant_login FROM veyndo_authority.keys k WHERE k.active;
END;
$$;

CREATE FUNCTION public.veyndo_scope_status() RETURNS jsonb
    LANGUAGE sql STABLE SECURITY DEFINER
    SET search_path TO 'pg_catalog', 'pg_temp'
    AS $$
 SELECT jsonb_build_object('enforced',enforced,'tenantRole',tenant_login,'systemRole',system_role)
 FROM veyndo_authority.configuration;
$$;

CREATE FUNCTION veyndo_authority.mac(payload bytea, secret bytea) RETURNS bytea
    LANGUAGE sql IMMUTABLE STRICT
    SET search_path TO 'pg_catalog', 'pg_temp'
    AS $$SELECT public.hmac(payload,secret,'sha256')$$;

CREATE FUNCTION veyndo_authority.random_key() RETURNS bytea
    LANGUAGE sql
    SET search_path TO 'pg_catalog', 'pg_temp'
    AS $$SELECT public.gen_random_bytes(32)$$;

CREATE INDEX app_action_log_cursor ON public.app_action_log USING btree (tenant, app, id DESC);

CREATE INDEX app_deliveries_dispatch ON public.app_deliveries USING btree (state, available_at, tenant, app, event_id) WHERE (state = ANY (ARRAY['queued'::text, 'running'::text]));

CREATE INDEX app_deliveries_event_retention ON public.app_deliveries USING btree (event_id);

CREATE INDEX app_deliveries_tenant_running ON public.app_deliveries USING btree (tenant, lease_until) WHERE (state = 'running'::text);

CREATE INDEX app_delivery_fairness ON public.app_deliveries USING btree (tenant, app, last_attempt_at DESC) WHERE (last_attempt_at IS NOT NULL);

CREATE INDEX app_evidence_search ON public.app_evidence USING gin (to_tsvector('simple'::regconfig, ((title || ' '::text) || body)));

CREATE INDEX app_export_poll ON public.app_export_cursors USING btree (last_poll, tenant, app);

CREATE INDEX app_jobs_active ON public.app_jobs USING btree (tenant, app, created_at DESC);

CREATE INDEX app_record_lookup ON public.app_record_indexes USING btree (tenant, app, entity, field, value_hash, record_id);

CREATE INDEX app_record_reference_asset ON public.app_record_references USING btree (tenant, asset_id) WHERE (asset_id IS NOT NULL);

CREATE INDEX app_record_reference_channel ON public.app_record_references USING btree (tenant, channel_id) WHERE (channel_id IS NOT NULL);

CREATE INDEX app_record_reference_company ON public.app_record_references USING btree (tenant, company_id) WHERE (company_id IS NOT NULL);

CREATE INDEX app_record_reference_customer ON public.app_record_references USING btree (tenant, customer_id) WHERE (customer_id IS NOT NULL);

CREATE INDEX app_record_reference_order ON public.app_record_references USING btree (tenant, order_id) WHERE (order_id IS NOT NULL);

CREATE INDEX app_record_reference_product ON public.app_record_references USING btree (tenant, product_id) WHERE (product_id IS NOT NULL);

CREATE INDEX app_record_reference_target ON public.app_record_references USING btree (tenant, app, target_entity, target_id);

CREATE INDEX app_record_reference_unit ON public.app_record_references USING btree (tenant, unit_id) WHERE (unit_id IS NOT NULL);

CREATE INDEX app_record_versions_retention ON public.app_record_versions USING btree (tenant, app, created_at);

CREATE INDEX app_schedule_runs_event_retention ON public.app_schedule_runs USING btree (event_id);

CREATE INDEX app_schedules_due ON public.app_schedules USING btree (next_run);

CREATE INDEX app_surface_grant_expiry ON public.app_surface_grants USING btree (tenant, app, expires_at);

CREATE INDEX app_webhook_receipts_event_retention ON public.app_webhook_receipts USING btree (event_id);

CREATE INDEX approved_reviews ON public.product_reviews USING btree (tenant, product_id, created_at DESC) WHERE approved;

CREATE INDEX b2b_approval_inbox ON public.b2b_approvals USING btree (tenant, unit_id, state, created_at DESC);

CREATE INDEX b2b_budget_usage ON public.b2b_budget_allocations USING btree (tenant, unit_id, period_start, currency) WHERE (NOT released);

CREATE INDEX b2b_customer_membership ON public.b2b_members USING btree (tenant, customer_id, unit_id);

CREATE INDEX b2b_lists_unit ON public.b2b_lists USING btree (tenant, unit_id, id);

CREATE INDEX b2b_quotes_buyer ON public.b2b_quotes USING btree (tenant, unit_id, buyer_id, created_at DESC);

CREATE INDEX behavior_expiry ON public.session_signal_events USING btree (expires_at) WHERE (purpose = 'analytics'::text);

CREATE INDEX behavior_journey ON public.session_signal_events USING btree (tenant, session, created_at DESC) WHERE (purpose = 'analytics'::text);

CREATE INDEX behavior_window ON public.session_signal_events USING btree (tenant, purpose, created_at DESC) WHERE (purpose = 'analytics'::text);

CREATE INDEX carts_sales_channel_lookup ON public.carts USING btree (tenant, sales_channel_id);

CREATE INDEX category_children ON public.categories USING btree (tenant, parent_id, "position", id);

CREATE INDEX category_products ON public.product_categories USING btree (tenant, category_id, product_id);

CREATE INDEX channel_previews_expiry ON public.channel_previews USING btree (expires_at);

CREATE INDEX chat_history ON public.chat_messages USING btree (tenant, conversation_id, id);

CREATE INDEX checkout_handoff_expiry ON public.checkout_handoffs USING btree (expires_at);

CREATE INDEX cognitive_chunk_keyset ON public.knowledge_chunks USING btree (tenant, (((document_id || ':'::text) || "position")));

CREATE INDEX connector_changes_scope ON public.connector_changes USING btree (tenant, app, seq);

CREATE INDEX connector_job_claim ON public.connector_jobs USING btree (app, available, tenant) WHERE (state = 'queued'::text);

CREATE INDEX connector_job_leases ON public.connector_jobs USING btree (lease_until) WHERE (state = 'running'::text);

CREATE INDEX consumer_requests_scope ON public.consumer_requests USING btree (tenant, created_at DESC, id);

CREATE INDEX conversations_actor ON public.conversations USING btree (tenant, owner_id, created_at DESC);

CREATE INDEX currency_price_work ON public.currency_price_jobs USING btree (created_at) WHERE (state = 'queued'::text);

CREATE INDEX customer_addresses_owner ON public.customer_addresses USING btree (tenant, email, id);

CREATE INDEX customer_order_lookup ON public.carts USING btree (tenant, ((data ->> 'email'::text)), id);

CREATE INDEX customer_sessions_expiry ON public.customer_sessions USING btree (expires_at);

CREATE INDEX customers_identity_lookup ON public.customers USING gin (lower(((((((((email || ' '::text) || COALESCE((profile ->> 'name'::text), ''::text)) || ' '::text) || COALESCE((profile ->> 'firstName'::text), ''::text)) || ' '::text) || COALESCE((profile ->> 'lastName'::text), ''::text)) || ' '::text) || COALESCE(customer_number, ''::text))) public.gin_trgm_ops);

CREATE INDEX customers_sales_channel_lookup ON public.customers USING btree (tenant, sales_channel_id);

CREATE INDEX developer_builds_library ON public.developer_builds USING btree (tenant, archived, created_at DESC);

CREATE INDEX embedding_generations_pending ON public.embedding_generations USING btree (model, tenant) WHERE (phase <> 'complete'::text);

CREATE INDEX embedding_jobs_diagnostics ON public.embedding_jobs USING btree (tenant, error_code) WHERE (error_code IS NOT NULL);

CREATE INDEX embedding_jobs_ready ON public.embedding_jobs USING btree (available_at, tenant) WHERE (attempts < 8);

CREATE INDEX entity_history_lookup ON public.entity_history USING btree (tenant, entity, entity_id, id DESC);

CREATE INDEX environments_live ON public.shop_environments USING btree (live_tenant);

CREATE INDEX flow_jobs_event_retention ON public.flow_jobs USING btree (event_id);

CREATE INDEX flow_jobs_expired_lease ON public.flow_jobs USING btree (lease_until) WHERE (state = 'running'::text);

CREATE INDEX flow_jobs_ready ON public.flow_jobs USING btree (available_at, event_id) WHERE (state = 'queued'::text);

CREATE INDEX flow_jobs_tenant_recent ON public.flow_jobs USING btree (tenant, event_id DESC, id);

CREATE INDEX hosted_frontends_app ON public.hosted_frontends USING btree (tenant, app_id) WHERE (app_id IS NOT NULL);

CREATE INDEX hosted_frontends_tenant ON public.hosted_frontends USING btree (tenant);

CREATE INDEX integration_app_active ON public.integration_keys USING btree (tenant, app_id, expires_at) WHERE (app_id IS NOT NULL);

CREATE INDEX integration_keys_identity ON public.integration_keys USING btree (user_id, tenant);

CREATE INDEX intelligence_assignments_time ON public.intelligence_assignments USING btree (tenant, experiment_id, created_at);

CREATE INDEX intelligence_live_payment_cart ON public.payment_attempts USING btree (tenant, order_id) WHERE ((environment = 'live'::text) AND (state = ANY (ARRAY['captured'::text, 'captured_late'::text, 'refunded'::text])));

CREATE INDEX knowledge_chunk_lexemes_source ON public.knowledge_chunk_lexemes USING btree (tenant, document_id, "position");

CREATE INDEX knowledge_chunk_search ON public.knowledge_chunks USING gin (to_tsvector('simple'::regconfig, text));

CREATE INDEX knowledge_chunks_language ON public.knowledge_chunks USING btree (tenant, document_id, locale);

CREATE INDEX knowledge_claim_lookup ON public.knowledge_relations USING btree (tenant, source_id, state) WHERE (kind = 'CLAIMS'::text);

CREATE INDEX knowledge_confirmed_query_text ON public.knowledge_relations USING gin (lower((data ->> 'text'::text)) public.gin_trgm_ops) WHERE ((kind = 'CLAIMS'::text) AND (state = 'confirmed'::text));

CREATE INDEX knowledge_confirmed_query_type ON public.knowledge_relations USING btree (tenant, target_kind, confidence) WHERE ((kind = 'CLAIMS'::text) AND (state = 'confirmed'::text));

CREATE INDEX knowledge_documents_workspace ON public.knowledge_documents USING btree (tenant, archived, id);

CREATE INDEX knowledge_product_lexemes_source ON public.knowledge_product_lexemes USING btree (tenant, product_id);

CREATE INDEX knowledge_receipts_event_retention ON public.knowledge_receipts USING btree (event_id);

CREATE INDEX knowledge_relations_target ON public.knowledge_relations USING btree (tenant, kind, target_id);

CREATE INDEX knowledge_serves_target ON public.knowledge_relations USING btree (tenant, target_id, source_id) WHERE (kind = 'SERVES'::text);

CREATE INDEX live_sessions ON public.user_sessions USING btree (user_id, expires_at);

CREATE INDEX media_jobs_queue ON public.media_jobs USING btree (created_at) WHERE (state = 'queued'::text);

CREATE INDEX membership_tenant ON public.memberships USING btree (tenant, user_id) WHERE active;

CREATE INDEX merchant_mfa_challenge_expiry ON public.merchant_mfa_challenges USING btree (expires_at);

CREATE INDEX observed_pairs_event_retention ON public.observed_pairs USING btree (last_event);

CREATE INDEX order_activity_scope ON public.order_activity USING btree (tenant, order_id, id);

CREATE INDEX order_chronological_page ON public.orders USING btree (tenant, created_at DESC, id DESC);

CREATE INDEX order_customer_rule_history ON public.orders USING btree (tenant, (((data -> 'orderCustomer'::text) ->> 'customerId'::text)), created_at);

CREATE INDEX order_receipts_scope ON public.order_receipts USING btree (tenant, order_id, created_at);

CREATE INDEX orders_customer_id ON public.orders USING btree (tenant, (((data -> 'orderCustomer'::text) ->> 'customerId'::text)), created_at DESC);

CREATE INDEX orders_guest_contact ON public.orders USING btree (tenant, (((data -> 'orderCustomer'::text) ->> 'email'::text)), created_at DESC, id DESC) WHERE (((data -> 'orderCustomer'::text) ->> 'guest'::text) = 'true'::text);

CREATE INDEX outbox_checkpoint_ready ON public.outbox_checkpoints USING btree (subscriber, served_order, tenant);

CREATE INDEX outbox_metadata_expiry ON public.outbox USING btree (delivered_at, id) WHERE (payload_retired_at IS NOT NULL);

CREATE INDEX outbox_pending ON public.outbox USING btree (id) WHERE (delivered_at IS NULL);

CREATE INDEX outbox_pending_apps ON public.outbox USING btree (tenant, id) WHERE ((delivered_at IS NULL) AND (payload_retired_at IS NULL) AND ((subscriber_mask & (2)::smallint) = 0));

CREATE INDEX outbox_pending_audit ON public.outbox USING btree (tenant, id) WHERE ((delivered_at IS NULL) AND (payload_retired_at IS NULL) AND ((subscriber_mask & (8)::smallint) = 0));

CREATE INDEX outbox_pending_cognition ON public.outbox USING btree (tenant, id) WHERE ((delivered_at IS NULL) AND (payload_retired_at IS NULL) AND ((subscriber_mask & (1)::smallint) = 0));

CREATE INDEX outbox_pending_flows ON public.outbox USING btree (tenant, id) WHERE ((delivered_at IS NULL) AND (payload_retired_at IS NULL) AND ((subscriber_mask & (4)::smallint) = 0));

CREATE INDEX outbox_ready ON public.outbox USING btree (available_at, id) WHERE ((delivered_at IS NULL) AND (dead_letter_at IS NULL));

CREATE INDEX outbox_retention ON public.outbox USING btree (delivered_at, id) WHERE ((payload_retired_at IS NULL) AND (delivered_at IS NOT NULL));

CREATE INDEX outbox_tenant_dispatch_order ON public.outbox_tenant_dispatch USING btree (served_order, tenant);

CREATE INDEX outbox_tenant_ready ON public.outbox USING btree (tenant, id) WHERE ((delivered_at IS NULL) AND (dead_letter_at IS NULL));

CREATE INDEX pair_evidence_event_retention ON public.pair_evidence USING btree (event_id);

CREATE INDEX payment_instruments_customer ON public.payment_instruments USING btree (tenant, customer_id, state, created_at);

CREATE INDEX payment_jobs_pending ON public.payment_jobs USING btree (available_at) WHERE (state = ANY (ARRAY['queued'::text, 'running'::text]));

CREATE INDEX platform_audit_time ON public.platform_audit USING btree (created_at DESC, id DESC);

CREATE INDEX platform_orders_window ON public.orders USING btree (created_at, tenant);

CREATE INDEX privacy_consent_log_scope ON public.privacy_consent_log USING btree (tenant, cart_id, created_at DESC);

CREATE INDEX privacy_consents_source ON public.privacy_consents USING btree (tenant, source_cart_id, channel_id) WHERE (source_cart_id IS NOT NULL);

CREATE INDEX product_admin_name ON public.products USING gin (lower(name) public.gin_trgm_ops);

CREATE INDEX product_admin_number ON public.products USING gin (lower(product_number) public.gin_trgm_ops);

CREATE INDEX product_admin_translation_name ON public.product_translations USING gin (lower(COALESCE(name, ''::text)) public.gin_trgm_ops);

CREATE INDEX product_assets_scope ON public.product_assets USING btree (tenant, product_id, kind);

CREATE INDEX product_category_page ON public.products USING btree (tenant, category, id) WHERE (parent_id IS NULL);

CREATE INDEX product_channel_hidden ON public.product_channel_visibility USING btree (tenant, channel_id, product_id) WHERE (NOT visible);

CREATE INDEX product_family ON public.products USING btree (tenant, parent_id);

CREATE INDEX product_root_page ON public.products USING btree (tenant, id) WHERE (parent_id IS NULL);

CREATE INDEX product_search_substring ON public.products USING gin (lower(((name || ' '::text) || description)) public.gin_trgm_ops) WHERE (parent_id IS NULL);

CREATE INDEX product_variant_page ON public.products USING btree (tenant, parent_id, id) WHERE (parent_id IS NOT NULL);

CREATE INDEX products_search ON public.products USING gin (to_tsvector('simple'::regconfig, ((name || ' '::text) || description)));

CREATE INDEX resource_lease_scope ON public.resource_leases USING btree (tenant, class, expires_at);

CREATE INDEX review_customer_rule_history ON public.product_reviews USING btree (tenant, session);

CREATE INDEX tasks_actor_recent ON public.tasks USING btree (tenant, run_actor, created_at DESC);

CREATE INDEX tasks_run_claim ON public.tasks USING btree (run_updated, tenant, id) WHERE (run_state = ANY (ARRAY['queued'::text, 'running'::text]));

CREATE INDEX tasks_tenant_active_run ON public.tasks USING btree (tenant, run_state) WHERE (run_state = ANY (ARRAY['queued'::text, 'running'::text, 'uncertain'::text]));

CREATE INDEX tenant_exposure_date ON public.exposures USING btree (tenant, created_at);

CREATE INDEX tenant_recent_orders ON public.orders USING btree (tenant, created_at DESC, id);

CREATE INDEX translation_items_ready ON public.translation_items USING btree (tenant, job_id, product_id) WHERE (status = 'ready'::text);

CREATE INDEX translation_jobs_run ON public.translation_jobs USING btree (tenant, run_task) WHERE (run_task IS NOT NULL);

CREATE INDEX translation_jobs_work ON public.translation_jobs USING btree (created_at) WHERE (status = ANY (ARRAY['queued'::text, 'processing'::text]));

CREATE INDEX translation_search_substring ON public.product_translations USING gin (lower(((COALESCE(name, ''::text) || ' '::text) || COALESCE(description, ''::text))) public.gin_trgm_ops);

CREATE POLICY app_record_version_scope ON public.app_record_versions USING ((tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))) WITH CHECK ((tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)));

CREATE POLICY authenticated_member ON public.merchant_users USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (EXISTS ( SELECT 1
   FROM public.memberships m
  WHERE ((m.user_id = merchant_users.id) AND (m.tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))))))) WITH CHECK (COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false));

CREATE POLICY authenticated_scope ON public.ai_price_budget AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_action_log AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_deliveries AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_evidence AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_export_cursors AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_jobs AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_models AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_packages AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_record_indexes AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_record_references AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_record_versions AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_records AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_schedule_runs AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_schedules AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_schema_history AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_service_secrets AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_storage_usage AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_surface_grants AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_versions AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_webhook_receipts AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.b2b_approvals AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.b2b_budget_allocations AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.b2b_catalog AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.b2b_companies AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.b2b_lists AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.b2b_members AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.b2b_quotes AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.b2b_units AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.carts AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.categories AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.channel_metrics AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.channel_previews AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.chat_messages AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.checkout_handoffs AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.commerce_customer_groups AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.commerce_flows AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.commerce_overrides AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.commerce_promotions AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.commerce_rules AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.commerce_settings AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.company_logos AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.company_overrides AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.consumer_request_reviews AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.consumer_requests AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.conversations AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.currency_price_jobs AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.customer_addresses AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.customer_sessions AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.customers AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.developer_builds AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.developer_drafts AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.embedding_generations AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.embedding_jobs AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.entity_history AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.experiences AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.exposures AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.extensions AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.flow_jobs AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.hosted_frontends AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.integration_keys AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.intelligence_assignments AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.intelligence_experiments AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.inventory_reservations AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.knowledge_chunk_lexemes AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.knowledge_chunks AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.knowledge_documents AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.knowledge_hypotheses AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.knowledge_index_status AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.knowledge_nodes AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.knowledge_product_lexemes AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.knowledge_receipts AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.knowledge_relation_history AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.knowledge_relations AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.legal_acceptances AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.media_jobs AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.memberships AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.merchant_handoffs AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.merchant_mfa_challenges AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.observed_pairs AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.order_activity AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.order_downloads AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.order_inventory_reservations AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.order_receipts AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.order_state_machines AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.order_transition_requests AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.orders AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.outbox AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.outbox_checkpoints AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.outbox_subscriber_failures AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.outbox_tenant_dispatch AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.pair_evidence AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.payment_attempts AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.payment_inbox AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.payment_instruments AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.payment_jobs AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.payment_provider_accounts AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.payment_receipt_claims AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.payment_refunds AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.platform_audit AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.policy AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.privacy_consent_log AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.privacy_consents AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.private_preferences AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.product_assets AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.product_categories AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.product_channel_visibility AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.product_reviews AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.product_translations AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.products AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.projections AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.promotion_uses AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.receipt_counters AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.receipt_settings AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.resource_leases AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.sales_channels AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.semantic_products AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.session_signal_events AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.session_signals AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.shop_environments AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.studio_workspace_versions AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR ((tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))) AND (user_id = ( SELECT (public.veyndo_scope_guard() ->> 'personalActor'::text)))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR ((tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))) AND (user_id = ( SELECT (public.veyndo_scope_guard() ->> 'personalActor'::text))))));

CREATE POLICY authenticated_scope ON public.studio_workspaces AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR ((tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))) AND (user_id = ( SELECT (public.veyndo_scope_guard() ->> 'personalActor'::text)))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR ((tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))) AND (user_id = ( SELECT (public.veyndo_scope_guard() ->> 'personalActor'::text))))));

CREATE POLICY authenticated_scope ON public.tasks AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.tenant_ai_usage AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.tenant_resource_limits AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.translation_items AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.translation_jobs AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.user_invites AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.vector_index_queue AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.workspace_roles AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_tenant ON public.tenants USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (id = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK (COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false));

CREATE POLICY channel_preview_tenant ON public.channel_previews USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = current_setting('rac.tenant'::text, true)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = current_setting('rac.tenant'::text, true))));

CREATE POLICY connector_scope ON public.connector_changes USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY connector_scope ON public.connector_config USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY connector_scope ON public.connector_jobs USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY connector_scope ON public.connector_limits USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY connector_scope ON public.connector_oauth USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY connector_scope ON public.connector_sources USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.ai_price_budget USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.app_deliveries USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.app_evidence USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.app_export_cursors USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.app_models USING (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text))) WITH CHECK (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text)));

CREATE POLICY core_tenant_scope ON public.app_packages USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.app_record_indexes USING (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text))) WITH CHECK (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text)));

CREATE POLICY core_tenant_scope ON public.app_record_references USING (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text))) WITH CHECK (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text)));

CREATE POLICY core_tenant_scope ON public.app_records USING (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text))) WITH CHECK (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text)));

CREATE POLICY core_tenant_scope ON public.app_schedule_runs USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.app_schedules USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.app_versions USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.app_webhook_receipts USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.b2b_approvals USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.b2b_budget_allocations USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.b2b_catalog USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.b2b_companies USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.b2b_lists USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.b2b_members USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.b2b_quotes USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.b2b_units USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.carts USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.categories USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.channel_metrics USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.chat_messages USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.checkout_handoffs USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.commerce_customer_groups USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.commerce_flows USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.commerce_overrides USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.commerce_promotions USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.commerce_rules USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.commerce_settings USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.company_logos USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.company_overrides USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.consumer_request_reviews USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.consumer_requests USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.conversations USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.customer_addresses USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.customer_sessions USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.customers USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.developer_builds USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.embedding_generations USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.embedding_jobs USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.entity_history USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.experiences USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.exposures USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.extensions USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.flow_jobs USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.hosted_frontends USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.integration_keys USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.intelligence_assignments USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.intelligence_experiments USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.inventory_reservations USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.knowledge_chunk_lexemes USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.knowledge_chunks USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.knowledge_documents USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.knowledge_hypotheses USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.knowledge_index_status USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.knowledge_nodes USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.knowledge_product_lexemes USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.knowledge_receipts USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.knowledge_relation_history USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.knowledge_relations USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.legal_acceptances USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.media_jobs USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.memberships USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.merchant_handoffs USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.merchant_mfa_challenges USING ((current_setting('rac.system'::text, true) = 'on'::text)) WITH CHECK ((current_setting('rac.system'::text, true) = 'on'::text));

CREATE POLICY core_tenant_scope ON public.observed_pairs USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.order_activity USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.order_downloads USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.order_inventory_reservations USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.order_receipts USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.order_state_machines USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.order_transition_requests USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.orders USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.outbox USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.outbox_checkpoints USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.outbox_subscriber_failures USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.outbox_tenant_dispatch USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.pair_evidence USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.payment_attempts USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.payment_inbox USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.payment_instruments USING (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text))) WITH CHECK (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text)));

CREATE POLICY core_tenant_scope ON public.payment_jobs USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.payment_provider_accounts USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.payment_receipt_claims USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.payment_refunds USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.platform_audit USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.policy USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.privacy_consent_log USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.privacy_consents USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.private_preferences USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.product_assets USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.product_categories USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.product_channel_visibility USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.product_reviews USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.product_translations USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.products USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.projections USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.promotion_uses USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.receipt_counters USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.receipt_settings USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.resource_leases USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.sales_channels USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.semantic_products USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.session_signal_events USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.session_signals USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.shop_environments USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.tasks USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.tenant_ai_usage USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.tenant_resource_limits USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK ((current_setting('rac.system'::text, true) = 'on'::text));

CREATE POLICY core_tenant_scope ON public.translation_items USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.translation_jobs USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.user_invites USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.vector_index_queue USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.workspace_roles USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY currency_price_scope ON public.currency_price_jobs USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY identity_system ON public.merchant_mfa USING ((current_setting('rac.system'::text, true) = 'on'::text)) WITH CHECK ((current_setting('rac.system'::text, true) = 'on'::text));

CREATE POLICY managed_write_delete ON public.app_models AS RESTRICTIVE FOR DELETE USING (COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false));

CREATE POLICY managed_write_delete ON public.app_record_indexes AS RESTRICTIVE FOR DELETE USING ((pg_trigger_depth() > 0));

CREATE POLICY managed_write_delete ON public.app_record_references AS RESTRICTIVE FOR DELETE USING ((pg_trigger_depth() > 0));

CREATE POLICY managed_write_insert ON public.app_models AS RESTRICTIVE FOR INSERT WITH CHECK (COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false));

CREATE POLICY managed_write_insert ON public.app_record_indexes AS RESTRICTIVE FOR INSERT WITH CHECK ((pg_trigger_depth() > 0));

CREATE POLICY managed_write_insert ON public.app_record_references AS RESTRICTIVE FOR INSERT WITH CHECK ((pg_trigger_depth() > 0));

CREATE POLICY managed_write_update ON public.app_models AS RESTRICTIVE FOR UPDATE USING (COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false)) WITH CHECK (COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false));

CREATE POLICY managed_write_update ON public.app_record_indexes AS RESTRICTIVE FOR UPDATE USING ((pg_trigger_depth() > 0)) WITH CHECK ((pg_trigger_depth() > 0));

CREATE POLICY managed_write_update ON public.app_record_references AS RESTRICTIVE FOR UPDATE USING ((pg_trigger_depth() > 0)) WITH CHECK ((pg_trigger_depth() > 0));

CREATE POLICY personal_workspace ON public.studio_workspaces USING (((tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)) AND (user_id = NULLIF(current_setting('rac.actor'::text, true), ''::text)))) WITH CHECK (((tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)) AND (user_id = NULLIF(current_setting('rac.actor'::text, true), ''::text))));

CREATE POLICY personal_workspace_version ON public.studio_workspace_versions USING (((tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)) AND (user_id = NULLIF(current_setting('rac.actor'::text, true), ''::text)))) WITH CHECK (((tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)) AND (user_id = NULLIF(current_setting('rac.actor'::text, true), ''::text))));

CREATE POLICY tenant_scope ON public.app_action_log USING (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text))) WITH CHECK (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text)));

CREATE POLICY tenant_scope ON public.app_jobs USING ((tenant = current_setting('rac.tenant'::text, true))) WITH CHECK ((tenant = current_setting('rac.tenant'::text, true)));

CREATE POLICY tenant_scope ON public.app_schema_history USING ((tenant = current_setting('rac.tenant'::text, true))) WITH CHECK ((tenant = current_setting('rac.tenant'::text, true)));

CREATE POLICY tenant_scope ON public.app_service_secrets USING (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text))) WITH CHECK (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text)));

CREATE POLICY tenant_scope ON public.app_storage_usage USING (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text))) WITH CHECK (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text)));

CREATE POLICY tenant_scope ON public.app_surface_grants USING (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text))) WITH CHECK (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text)));

CREATE POLICY tenant_scope ON public.developer_drafts USING (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text))) WITH CHECK (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text)));

CREATE SCHEMA veyndo_authority;

CREATE SEQUENCE public.app_action_log_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

CREATE SEQUENCE public.chat_messages_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

CREATE SEQUENCE public.entity_history_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

CREATE SEQUENCE public.order_activity_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

CREATE SEQUENCE public.outbox_dispatch_sequence
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

CREATE SEQUENCE public.outbox_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

CREATE SEQUENCE public.platform_audit_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

CREATE TABLE public.ai_price_budget (
    tenant text NOT NULL,
    day date NOT NULL,
    product_id text NOT NULL,
    baseline_minor bigint NOT NULL,
    currency text NOT NULL,
    task_id text NOT NULL,
    actor text NOT NULL,
    CONSTRAINT ai_price_budget_baseline_minor_check CHECK ((baseline_minor >= 0))
);

CREATE TABLE public.app_action_log (
    id bigint NOT NULL,
    tenant text NOT NULL,
    app text NOT NULL,
    action text NOT NULL,
    actor text NOT NULL,
    status integer NOT NULL,
    duration_ms bigint NOT NULL,
    input_digest text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.app_deliveries (
    tenant text NOT NULL,
    app text NOT NULL,
    event_id bigint NOT NULL,
    state text DEFAULT 'queued'::text NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    lease_until timestamp with time zone,
    available_at timestamp with time zone DEFAULT now() NOT NULL,
    error text,
    lease_token uuid,
    last_attempt_at timestamp with time zone
);

CREATE TABLE public.app_evidence (
    tenant text NOT NULL,
    app text NOT NULL,
    source_id text NOT NULL,
    kind text NOT NULL,
    title text NOT NULL,
    body text NOT NULL,
    source_url text NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    digest text NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.app_export_cursors (
    tenant text NOT NULL,
    app text NOT NULL,
    cursor bigint DEFAULT 0 NOT NULL,
    last_poll timestamp with time zone DEFAULT '1970-01-01 00:00:00+00'::timestamp with time zone NOT NULL
);

CREATE TABLE public.app_jobs (
    tenant text NOT NULL,
    app text NOT NULL,
    id text NOT NULL,
    action text NOT NULL,
    actor text NOT NULL,
    permission text NOT NULL,
    package_digest text NOT NULL,
    request_key text NOT NULL,
    input_digest text NOT NULL,
    input jsonb NOT NULL,
    result jsonb,
    status text DEFAULT 'queued'::text NOT NULL,
    progress integer DEFAULT 0 NOT NULL,
    message jsonb DEFAULT '{}'::jsonb NOT NULL,
    lease text,
    lease_until timestamp with time zone,
    started_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    CONSTRAINT app_jobs_input_check CHECK ((octet_length((input)::text) <= 20000)),
    CONSTRAINT app_jobs_progress_check CHECK (((progress >= 0) AND (progress <= 100))),
    CONSTRAINT app_jobs_result_check CHECK (((result IS NULL) OR (octet_length((result)::text) <= 40000))),
    CONSTRAINT app_jobs_status_check CHECK ((status = ANY (ARRAY['queued'::text, 'running'::text, 'succeeded'::text, 'failed'::text, 'uncertain'::text, 'cancel_requested'::text, 'cancelled'::text])))
);

CREATE TABLE public.app_models (
    tenant text NOT NULL,
    app text NOT NULL,
    entity text NOT NULL,
    version text NOT NULL,
    definition jsonb NOT NULL
);

CREATE TABLE public.app_packages (
    tenant text NOT NULL,
    id text NOT NULL,
    version text NOT NULL,
    manifest jsonb NOT NULL,
    digest text NOT NULL,
    active boolean DEFAULT true NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    installed_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.app_record_indexes (
    tenant text NOT NULL,
    app text NOT NULL,
    entity text NOT NULL,
    field text NOT NULL,
    record_id text NOT NULL,
    value jsonb NOT NULL,
    value_hash bytea GENERATED ALWAYS AS (sha256(jsonb_send(value))) STORED,
    is_unique boolean NOT NULL
);

CREATE TABLE public.app_record_references (
    tenant text NOT NULL,
    app text NOT NULL,
    entity text NOT NULL,
    record_id text NOT NULL,
    field text NOT NULL,
    "position" integer NOT NULL,
    target_entity text,
    target_id text,
    product_id text,
    customer_id text,
    order_id text,
    channel_id text,
    company_id text,
    unit_id text,
    asset_id text,
    CONSTRAINT app_record_references_check CHECK ((num_nonnulls(target_id, product_id, customer_id, order_id, channel_id, company_id, unit_id, asset_id) = 1)),
    CONSTRAINT app_record_references_check1 CHECK (((target_entity IS NULL) = (target_id IS NULL))),
    CONSTRAINT app_record_references_position_check CHECK ((("position" >= 1) AND ("position" <= 100)))
);

CREATE TABLE public.app_record_versions (
    tenant text NOT NULL,
    app text NOT NULL,
    entity text NOT NULL,
    record_id text NOT NULL,
    revision bigint NOT NULL,
    fields jsonb NOT NULL,
    read_scopes jsonb NOT NULL,
    actor text,
    source text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.app_records (
    tenant text NOT NULL,
    app text NOT NULL,
    entity text NOT NULL,
    id text NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    version text NOT NULL,
    fields jsonb NOT NULL,
    storage_rows bigint DEFAULT 1 NOT NULL,
    storage_bytes bigint DEFAULT 0 NOT NULL,
    CONSTRAINT app_records_fields_check CHECK ((jsonb_typeof(fields) = 'object'::text)),
    CONSTRAINT app_records_revision_check CHECK ((revision > 0))
);

CREATE TABLE public.app_schedule_runs (
    tenant text NOT NULL,
    app text NOT NULL,
    schedule text NOT NULL,
    due timestamp with time zone NOT NULL,
    event_id bigint NOT NULL
);

CREATE TABLE public.app_schedules (
    tenant text NOT NULL,
    app text NOT NULL,
    id text NOT NULL,
    definition jsonb NOT NULL,
    next_run timestamp with time zone NOT NULL
);

CREATE TABLE public.app_schema_history (
    tenant text NOT NULL,
    app text NOT NULL,
    to_version text NOT NULL,
    from_version text NOT NULL,
    old_manifest jsonb NOT NULL,
    records jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT app_schema_history_records_check CHECK ((octet_length((records)::text) <= 4194304))
);

CREATE TABLE public.app_service_secrets (
    tenant text NOT NULL,
    app text NOT NULL,
    kind text NOT NULL,
    ciphertext text NOT NULL,
    package_digest text NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    actor text NOT NULL,
    rotated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT app_service_secrets_kind_check CHECK ((kind = ANY (ARRAY['service'::text, 'webhook'::text, 'outbound'::text, 'settings'::text]))),
    CONSTRAINT app_service_secrets_package_digest_check CHECK ((package_digest ~ '^[a-f0-9]{64}$'::text))
);

CREATE TABLE public.app_storage_usage (
    tenant text NOT NULL,
    app text NOT NULL,
    rows bigint DEFAULT 0 NOT NULL,
    bytes bigint DEFAULT 0 NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT app_storage_usage_bytes_check CHECK (((bytes >= 0) AND (bytes <= 67108864))),
    CONSTRAINT app_storage_usage_rows_check CHECK (((rows >= 0) AND (rows <= 100000)))
);

CREATE TABLE public.app_surface_grants (
    tenant text NOT NULL,
    token_hash text NOT NULL,
    app text NOT NULL,
    surface text NOT NULL,
    actor_hash text NOT NULL,
    package_digest text NOT NULL,
    actions jsonb NOT NULL,
    context jsonb NOT NULL,
    expires_at timestamp with time zone NOT NULL
);

CREATE TABLE public.app_versions (
    tenant text NOT NULL,
    app text NOT NULL,
    version text NOT NULL,
    digest text NOT NULL,
    manifest jsonb NOT NULL,
    installed_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.app_webhook_receipts (
    tenant text NOT NULL,
    app text NOT NULL,
    webhook text NOT NULL,
    request_key text NOT NULL,
    digest text NOT NULL,
    event_id bigint NOT NULL
);

CREATE TABLE public.auth_attempt_buckets (
    key text NOT NULL,
    window_at timestamp with time zone DEFAULT now() NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    blocked_until timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.b2b_approvals (
    tenant text NOT NULL,
    id text NOT NULL,
    unit_id text NOT NULL,
    buyer_id text NOT NULL,
    cart_id text NOT NULL,
    fingerprint text NOT NULL,
    data jsonb NOT NULL,
    state text DEFAULT 'pending'::text NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT b2b_approvals_revision_check CHECK ((revision > 0)),
    CONSTRAINT b2b_approvals_state_check CHECK ((state = ANY (ARRAY['pending'::text, 'approved'::text, 'rejected'::text, 'consumed'::text])))
);

CREATE TABLE public.b2b_budget_allocations (
    tenant text NOT NULL,
    order_id text NOT NULL,
    unit_id text NOT NULL,
    period_start date NOT NULL,
    currency text NOT NULL,
    amount_minor bigint NOT NULL,
    released boolean DEFAULT false NOT NULL,
    CONSTRAINT b2b_budget_allocations_amount_minor_check CHECK ((amount_minor >= 0))
);

CREATE TABLE public.b2b_catalog (
    tenant text NOT NULL,
    unit_id text NOT NULL,
    product_id text NOT NULL
);

CREATE TABLE public.b2b_companies (
    tenant text NOT NULL,
    id text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT b2b_companies_revision_check CHECK ((revision > 0))
);

CREATE TABLE public.b2b_lists (
    tenant text NOT NULL,
    id text NOT NULL,
    unit_id text NOT NULL,
    owner_id text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    CONSTRAINT b2b_lists_revision_check CHECK ((revision > 0))
);

CREATE TABLE public.b2b_members (
    tenant text NOT NULL,
    unit_id text NOT NULL,
    customer_id text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    CONSTRAINT b2b_members_revision_check CHECK ((revision > 0))
);

CREATE TABLE public.b2b_quotes (
    tenant text NOT NULL,
    id text NOT NULL,
    unit_id text NOT NULL,
    buyer_id text NOT NULL,
    cart_id text NOT NULL,
    state text DEFAULT 'requested'::text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT b2b_quotes_revision_check CHECK ((revision > 0)),
    CONSTRAINT b2b_quotes_state_check CHECK (((state = ANY (ARRAY['requested'::text, 'offered'::text, 'accepted'::text, 'ordered'::text, 'rejected'::text])) OR (state ~ '^a[0-9a-f]{8}_[a-z][a-z0-9_]{0,15}$'::text)))
);

CREATE TABLE public.b2b_units (
    tenant text NOT NULL,
    id text NOT NULL,
    company_id text NOT NULL,
    parent_id text,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    CONSTRAINT b2b_units_check CHECK ((parent_id IS DISTINCT FROM id)),
    CONSTRAINT b2b_units_revision_check CHECK ((revision > 0))
);

CREATE TABLE public.carts (
    id text NOT NULL,
    tenant text NOT NULL,
    token text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    status text DEFAULT 'open'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    sales_channel_id text GENERATED ALWAYS AS (COALESCE((data ->> 'sales_channel'::text), 'default'::text)) STORED
);

CREATE TABLE public.categories (
    tenant text NOT NULL,
    id text NOT NULL,
    parent_id text,
    "position" integer DEFAULT 0 NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    CONSTRAINT categories_check CHECK (((parent_id IS NULL) OR (parent_id <> id)))
);

CREATE TABLE public.channel_metrics (
    tenant text NOT NULL,
    channel text NOT NULL,
    calls bigint DEFAULT 0 NOT NULL,
    failures bigint DEFAULT 0 NOT NULL,
    last_seen timestamp with time zone DEFAULT now() NOT NULL,
    total_ms bigint DEFAULT 0 NOT NULL,
    max_ms bigint DEFAULT 0 NOT NULL,
    timed_calls bigint DEFAULT 0 NOT NULL,
    response_counts jsonb DEFAULT '{}'::jsonb NOT NULL
);

CREATE TABLE public.channel_previews (
    digest text NOT NULL,
    tenant text NOT NULL,
    channel text NOT NULL,
    session_digest text NOT NULL,
    channel_revision bigint NOT NULL,
    alias text,
    redeemed boolean DEFAULT false NOT NULL,
    expires_at timestamp with time zone NOT NULL
);

CREATE TABLE public.chat_messages (
    id bigint NOT NULL,
    tenant text NOT NULL,
    conversation_id text NOT NULL,
    role text NOT NULL,
    content text NOT NULL,
    data jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT chat_messages_role_check CHECK ((role = ANY (ARRAY['user'::text, 'assistant'::text, 'system'::text])))
);

CREATE TABLE public.checkout_handoffs (
    digest text NOT NULL,
    tenant text NOT NULL,
    cart_id text NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    recovery_digest text,
    recovery_origin text,
    recovery_expires_at timestamp with time zone,
    consumed_at timestamp with time zone
);

CREATE TABLE public.commerce_customer_groups (
    tenant text NOT NULL,
    id text NOT NULL,
    data jsonb DEFAULT '{}'::jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL
);

CREATE TABLE public.commerce_flows (
    tenant text NOT NULL,
    id text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL
);

CREATE TABLE public.commerce_migrations (
    version text NOT NULL,
    checksum text NOT NULL,
    applied_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.commerce_overrides (
    tenant text NOT NULL,
    channel_id text NOT NULL,
    data jsonb DEFAULT '[]'::jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    cache_version uuid DEFAULT gen_random_uuid() NOT NULL,
    CONSTRAINT commerce_overrides_data_check CHECK ((jsonb_typeof(data) = 'array'::text))
);

CREATE TABLE public.commerce_promotions (
    tenant text NOT NULL,
    id text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL
);

CREATE TABLE public.commerce_rules (
    tenant text NOT NULL,
    id text NOT NULL,
    name jsonb NOT NULL,
    condition jsonb NOT NULL,
    active boolean DEFAULT true NOT NULL,
    revision bigint DEFAULT 1 NOT NULL
);

CREATE TABLE public.commerce_settings (
    tenant text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    cache_version uuid DEFAULT gen_random_uuid() NOT NULL
);

CREATE TABLE public.company_logos (
    tenant text NOT NULL,
    id text NOT NULL,
    content bytea NOT NULL,
    digest text NOT NULL,
    mime text NOT NULL,
    width integer NOT NULL,
    height integer NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT company_logos_check CHECK ((((width >= 1) AND (width <= 4096)) AND ((height >= 1) AND (height <= 4096)))),
    CONSTRAINT company_logos_content_check CHECK (((octet_length(content) >= 1) AND (octet_length(content) <= 2097152))),
    CONSTRAINT company_logos_mime_check CHECK ((mime = 'image/png'::text))
);

CREATE TABLE public.company_overrides (
    tenant text NOT NULL,
    channel_id text NOT NULL,
    data jsonb DEFAULT '{}'::jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    CONSTRAINT company_overrides_data_check CHECK ((jsonb_typeof(data) = 'object'::text))
);

CREATE TABLE public.connector_changes (
    seq bigint NOT NULL,
    tenant text NOT NULL,
    app text NOT NULL,
    data bytea NOT NULL
);

CREATE TABLE public.connector_config (
    tenant text NOT NULL,
    app text NOT NULL,
    data bytea NOT NULL
);

CREATE TABLE public.connector_jobs (
    tenant text NOT NULL,
    app text NOT NULL,
    id text NOT NULL,
    fingerprint text NOT NULL,
    state text NOT NULL,
    payload bytea NOT NULL,
    result bytea,
    available timestamp with time zone DEFAULT now() NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    lease_id uuid,
    lease_until timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT connector_jobs_state_check CHECK ((state = ANY (ARRAY['queued'::text, 'running'::text, 'completed'::text, 'failed'::text, 'uncertain'::text])))
);

CREATE TABLE public.connector_limits (
    tenant text NOT NULL,
    app text NOT NULL,
    day date DEFAULT CURRENT_DATE NOT NULL,
    enqueued bigint DEFAULT 0 NOT NULL,
    minute timestamp with time zone DEFAULT date_trunc('minute'::text, now()) NOT NULL,
    dispatched bigint DEFAULT 0 NOT NULL,
    last_started timestamp with time zone DEFAULT '1970-01-01 00:00:00+00'::timestamp with time zone NOT NULL
);

CREATE TABLE public.connector_oauth (
    digest text NOT NULL,
    tenant text NOT NULL,
    app text NOT NULL,
    data bytea NOT NULL,
    expires_at timestamp with time zone NOT NULL
);

CREATE TABLE public.connector_sources (
    tenant text NOT NULL,
    app text NOT NULL,
    id text NOT NULL,
    data bytea NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.consumer_request_reviews (
    tenant text NOT NULL,
    request_id text NOT NULL,
    revision bigint NOT NULL,
    actor text NOT NULL,
    state text NOT NULL,
    note text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.consumer_requests (
    tenant text NOT NULL,
    id text NOT NULL,
    cart_id text NOT NULL,
    channel_id text NOT NULL,
    kind text NOT NULL,
    data jsonb NOT NULL,
    state text DEFAULT 'received'::text NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT consumer_requests_kind_check CHECK ((kind = ANY (ARRAY['withdrawal'::text, 'access'::text, 'erase'::text, 'correct'::text, 'portability'::text, 'objection'::text]))),
    CONSTRAINT consumer_requests_state_check CHECK ((state = ANY (ARRAY['received'::text, 'in_review'::text, 'completed'::text, 'declined'::text])))
);

CREATE TABLE public.conversations (
    tenant text NOT NULL,
    id text NOT NULL,
    title text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    turn_owner text,
    turn_until timestamp with time zone,
    owner_id text
);

CREATE TABLE public.currency_price_jobs (
    tenant text NOT NULL,
    id text NOT NULL,
    data jsonb NOT NULL,
    state text DEFAULT 'queued'::text NOT NULL,
    cursor text DEFAULT ''::text NOT NULL,
    ceiling text DEFAULT ''::text NOT NULL,
    processed bigint DEFAULT 0 NOT NULL,
    created_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    CONSTRAINT currency_price_jobs_state_check CHECK ((state = ANY (ARRAY['queued'::text, 'completed'::text, 'failed'::text])))
);

CREATE TABLE public.customer_addresses (
    tenant text NOT NULL,
    email text NOT NULL,
    id text NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    data jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.customer_sessions (
    digest text NOT NULL,
    tenant text NOT NULL,
    email text NOT NULL,
    expires_at timestamp with time zone DEFAULT (now() + '7 days'::interval) NOT NULL
);

CREATE TABLE public.customers (
    tenant text NOT NULL,
    email text NOT NULL,
    password_hash text NOT NULL,
    company text,
    group_name text DEFAULT 'consumer'::text NOT NULL,
    profile jsonb DEFAULT '{}'::jsonb NOT NULL,
    active boolean DEFAULT true NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    id text DEFAULT replace((gen_random_uuid())::text, '-'::text, ''::text) NOT NULL,
    customer_number text DEFAULT ('C-'::text || upper(substr(md5(((random())::text || (clock_timestamp())::text)), 1, 12))) NOT NULL,
    default_billing_address_id text,
    default_shipping_address_id text,
    sales_channel_id text DEFAULT 'default'::text NOT NULL,
    language_id text DEFAULT 'en-GB'::text NOT NULL,
    first_login timestamp with time zone,
    last_login timestamp with time zone,
    last_payment_method_id text,
    automation jsonb DEFAULT '{}'::jsonb NOT NULL
);

CREATE TABLE public.developer_builds (
    id text NOT NULL,
    tenant text NOT NULL,
    environment text NOT NULL,
    app text NOT NULL,
    version text NOT NULL,
    digest text NOT NULL,
    manifest jsonb NOT NULL,
    prompt text NOT NULL,
    provider text NOT NULL,
    model text,
    summary jsonb NOT NULL,
    state text DEFAULT 'draft'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    archived boolean DEFAULT false NOT NULL,
    archived_at timestamp with time zone,
    CONSTRAINT developer_builds_state_check CHECK ((state = ANY (ARRAY['draft'::text, 'staged'::text])))
);

CREATE TABLE public.developer_drafts (
    tenant text NOT NULL,
    actor_key text NOT NULL,
    id text NOT NULL,
    manifest jsonb NOT NULL,
    environment text,
    revision bigint DEFAULT 1 NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.embedding_generations (
    tenant text NOT NULL,
    model text NOT NULL,
    phase text DEFAULT 'product'::text NOT NULL,
    cursor text DEFAULT ''::text NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    model_revision bigint DEFAULT 0 NOT NULL,
    CONSTRAINT embedding_generations_model_check CHECK (((length(model) >= 1) AND (length(model) <= 256))),
    CONSTRAINT embedding_generations_model_revision_check CHECK ((model_revision >= 0)),
    CONSTRAINT embedding_generations_phase_check CHECK ((phase = ANY (ARRAY['product'::text, 'document'::text, 'complete'::text])))
);

CREATE TABLE public.embedding_jobs (
    tenant text NOT NULL,
    kind text NOT NULL,
    object_id text NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    available_at timestamp with time zone DEFAULT now() NOT NULL,
    lease text,
    lease_until timestamp with time zone,
    error_code text,
    CONSTRAINT embedding_jobs_kind_check CHECK ((kind = ANY (ARRAY['product'::text, 'document'::text])))
);

CREATE TABLE public.entity_history (
    id bigint NOT NULL,
    tenant text NOT NULL,
    entity text NOT NULL,
    entity_id text NOT NULL,
    transaction_id bigint NOT NULL,
    actor text,
    source text DEFAULT 'system'::text NOT NULL,
    reason text,
    before_state jsonb,
    after_state jsonb,
    created_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    finalized boolean DEFAULT false NOT NULL
);

CREATE TABLE public.experiences (
    tenant text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL
);

CREATE TABLE public.exposures (
    tenant text NOT NULL,
    session text NOT NULL,
    variant text NOT NULL,
    propensity double precision NOT NULL,
    rewarded boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.extensions (
    tenant text NOT NULL,
    wat text NOT NULL,
    digest text NOT NULL,
    revision bigint DEFAULT 1 NOT NULL
);

CREATE TABLE public.flow_jobs (
    id text NOT NULL,
    tenant text NOT NULL,
    flow text NOT NULL,
    event_id bigint NOT NULL,
    definition jsonb NOT NULL,
    state text DEFAULT 'queued'::text NOT NULL,
    lease_until timestamp with time zone,
    attempts integer DEFAULT 0 NOT NULL,
    result jsonb,
    error text,
    available_at timestamp with time zone DEFAULT now() NOT NULL,
    cursor text,
    execution jsonb DEFAULT '{}'::jsonb NOT NULL
);

CREATE TABLE public.flow_steps (
    job text NOT NULL,
    node text NOT NULL,
    state text DEFAULT 'running'::text NOT NULL,
    result jsonb,
    error text,
    started_at timestamp with time zone DEFAULT now() NOT NULL,
    finished_at timestamp with time zone
);

CREATE TABLE public.hosted_frontends (
    alias text NOT NULL,
    tenant text NOT NULL,
    channel text NOT NULL,
    origin text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    experience_alias text NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    app_id text
);

CREATE TABLE public.identity_inference_daily (
    email text NOT NULL,
    day date NOT NULL,
    calls integer NOT NULL
);

CREATE TABLE public.identity_nonces (
    digest text NOT NULL,
    expires_at timestamp with time zone NOT NULL
);

CREATE TABLE public.integration_keys (
    id text NOT NULL,
    tenant text NOT NULL,
    user_id text NOT NULL,
    digest text NOT NULL,
    name text NOT NULL,
    permissions jsonb NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    app_id text,
    package_digest text,
    app_permissions jsonb DEFAULT '[]'::jsonb NOT NULL,
    agent_budget jsonb,
    agent_tokens_allocated bigint DEFAULT 0 NOT NULL,
    agent_cost_allocated bigint DEFAULT 0 NOT NULL,
    CONSTRAINT agent_budget_shape CHECK (((agent_budget IS NULL) OR ((jsonb_typeof(agent_budget) = 'object'::text) AND (agent_budget ?& ARRAY['provider'::text, 'model'::text, 'tokens'::text, 'costMicros'::text, 'pricePerMillionMicros'::text]) AND ((agent_budget ->> 'provider'::text) = ANY (ARRAY['openai'::text, 'anthropic'::text, 'ollama'::text])) AND (jsonb_typeof((agent_budget -> 'model'::text)) = 'string'::text) AND ((length((agent_budget ->> 'model'::text)) >= 1) AND (length((agent_budget ->> 'model'::text)) <= 128)) AND (jsonb_typeof((agent_budget -> 'tokens'::text)) = 'number'::text) AND (jsonb_typeof((agent_budget -> 'costMicros'::text)) = 'number'::text) AND (jsonb_typeof((agent_budget -> 'pricePerMillionMicros'::text)) = 'number'::text) AND ((((agent_budget ->> 'tokens'::text))::bigint >= 1) AND (((agent_budget ->> 'tokens'::text))::bigint <= '1000000000000'::bigint)) AND ((((agent_budget ->> 'costMicros'::text))::bigint >= 1) AND (((agent_budget ->> 'costMicros'::text))::bigint <= '1000000000000'::bigint)) AND ((((agent_budget ->> 'pricePerMillionMicros'::text))::bigint >= 1) AND (((agent_budget ->> 'pricePerMillionMicros'::text))::bigint <= '1000000000000'::bigint)) AND (app_id IS NULL)))),
    CONSTRAINT integration_app_digest CHECK ((((app_id IS NULL) AND (package_digest IS NULL)) OR ((app_id IS NOT NULL) AND (package_digest ~ '^[a-f0-9]{64}$'::text)))),
    CONSTRAINT integration_keys_agent_cost_allocated_check CHECK ((agent_cost_allocated >= 0)),
    CONSTRAINT integration_keys_agent_tokens_allocated_check CHECK ((agent_tokens_allocated >= 0)),
    CONSTRAINT integration_keys_app_permissions_check CHECK ((jsonb_typeof(app_permissions) = 'array'::text))
);

CREATE TABLE public.intelligence_assignments (
    tenant text NOT NULL,
    experiment_id text NOT NULL,
    cart_id text NOT NULL,
    arm integer NOT NULL,
    baseline double precision NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT intelligence_assignments_arm_check CHECK ((arm = ANY (ARRAY[0, 1]))),
    CONSTRAINT intelligence_assignments_baseline_check CHECK (((baseline >= (0)::double precision) AND (baseline <= (1)::double precision)))
);

CREATE TABLE public.intelligence_experiments (
    tenant text NOT NULL,
    id text NOT NULL,
    channel text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    state text NOT NULL,
    withdrawn integer DEFAULT 0 NOT NULL,
    actor text NOT NULL,
    started_at timestamp with time zone,
    ends_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    final_report jsonb,
    CONSTRAINT intelligence_experiments_state_check CHECK ((state = ANY (ARRAY['draft'::text, 'running'::text, 'stopped'::text, 'completed'::text]))),
    CONSTRAINT intelligence_experiments_withdrawn_check CHECK ((withdrawn >= 0))
);

CREATE TABLE public.inventory_reservations (
    tenant text NOT NULL,
    attempt_id text NOT NULL,
    product_id text NOT NULL,
    quantity integer NOT NULL,
    released boolean DEFAULT false NOT NULL,
    CONSTRAINT inventory_reservations_quantity_check CHECK ((quantity > 0))
);

CREATE TABLE public.knowledge_chunk_lexemes (
    tenant text NOT NULL,
    document_id text NOT NULL,
    "position" integer NOT NULL,
    lexeme text NOT NULL
);

CREATE TABLE public.knowledge_chunks (
    tenant text NOT NULL,
    document_id text NOT NULL,
    "position" integer NOT NULL,
    text text NOT NULL,
    embedding real[],
    embedding_model text,
    locale text DEFAULT ''::text NOT NULL,
    CONSTRAINT cognitive_chunk_geometry CHECK (((embedding IS NULL) OR ((cardinality(embedding) >= 1) AND (cardinality(embedding) <= 8192))))
);

CREATE TABLE public.knowledge_documents (
    tenant text NOT NULL,
    id text NOT NULL,
    product_id text,
    title text NOT NULL,
    content_hash text NOT NULL,
    content text NOT NULL,
    visibility text DEFAULT 'private'::text NOT NULL,
    source_type text NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    kind text DEFAULT 'document'::text NOT NULL,
    locale text DEFAULT 'en-GB'::text NOT NULL,
    translations jsonb DEFAULT '{}'::jsonb NOT NULL,
    archived boolean DEFAULT false NOT NULL,
    CONSTRAINT knowledge_documents_visibility_check CHECK ((visibility = ANY (ARRAY['private'::text, 'public'::text])))
);

CREATE TABLE public.knowledge_hypotheses (
    tenant text NOT NULL,
    id text NOT NULL,
    kind text NOT NULL,
    evidence jsonb NOT NULL,
    state text DEFAULT 'proposed'::text NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT knowledge_hypotheses_state_check CHECK ((state = ANY (ARRAY['proposed'::text, 'dismissed'::text, 'experiment'::text, 'published'::text])))
);

CREATE TABLE public.knowledge_index_status (
    tenant text NOT NULL,
    indexed_products bigint DEFAULT 0 NOT NULL,
    pending_jobs bigint DEFAULT 0 NOT NULL,
    errors jsonb DEFAULT '{}'::jsonb NOT NULL,
    CONSTRAINT knowledge_index_status_indexed_products_check CHECK ((indexed_products >= 0)),
    CONSTRAINT knowledge_index_status_pending_jobs_check CHECK ((pending_jobs >= 0))
);

CREATE TABLE public.knowledge_nodes (
    tenant text NOT NULL,
    kind text NOT NULL,
    id text NOT NULL,
    data jsonb DEFAULT '{}'::jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    CONSTRAINT knowledge_nodes_check CHECK (((length(kind) <= 80) AND (length(id) <= 240)))
);

CREATE TABLE public.knowledge_product_lexemes (
    tenant text NOT NULL,
    product_id text NOT NULL,
    lexeme text NOT NULL
);

CREATE TABLE public.knowledge_receipts (
    tenant text NOT NULL,
    event_id bigint NOT NULL
);

CREATE TABLE public.knowledge_relation_history (
    tenant text NOT NULL,
    kind text NOT NULL,
    source_id text NOT NULL,
    target_id text NOT NULL,
    revision bigint NOT NULL,
    snapshot jsonb NOT NULL,
    recorded_from timestamp with time zone NOT NULL,
    recorded_until timestamp with time zone NOT NULL
);

CREATE TABLE public.knowledge_relations (
    tenant text NOT NULL,
    kind text NOT NULL,
    source_id text NOT NULL,
    target_id text NOT NULL,
    data jsonb DEFAULT '{}'::jsonb NOT NULL,
    source_kind text DEFAULT 'product'::text NOT NULL,
    target_kind text DEFAULT 'intent'::text NOT NULL,
    state text DEFAULT 'evidenced'::text NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    confidence numeric DEFAULT 1 NOT NULL,
    valid_from timestamp with time zone DEFAULT now() NOT NULL,
    valid_until timestamp with time zone,
    recorded_from timestamp with time zone DEFAULT now() NOT NULL,
    actor text DEFAULT 'system'::text NOT NULL,
    CONSTRAINT knowledge_relations_confidence_check CHECK (((confidence >= (0)::numeric) AND (confidence <= (1)::numeric))),
    CONSTRAINT knowledge_relations_state_check CHECK ((state = ANY (ARRAY['proposed'::text, 'evidenced'::text, 'confirmed'::text, 'rejected'::text]))),
    CONSTRAINT knowledge_relations_valid_time CHECK (((valid_until IS NULL) OR (valid_until > valid_from)))
);

CREATE TABLE public.languages (
    id text NOT NULL,
    locale text NOT NULL,
    parent_id text
);

CREATE TABLE public.legal_acceptances (
    tenant text NOT NULL,
    cart_id text NOT NULL,
    data jsonb NOT NULL
);

CREATE TABLE public.media_jobs (
    tenant text NOT NULL,
    id text NOT NULL,
    product_id text NOT NULL,
    product_revision bigint NOT NULL,
    request jsonb NOT NULL,
    state text DEFAULT 'queued'::text NOT NULL,
    asset_id text,
    error text,
    started_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT media_jobs_state_check CHECK ((state = ANY (ARRAY['queued'::text, 'processing'::text, 'ready'::text, 'failed'::text, 'applied'::text])))
);

CREATE TABLE public.memberships (
    user_id text NOT NULL,
    tenant text NOT NULL,
    role text NOT NULL,
    active boolean DEFAULT true NOT NULL,
    permissions jsonb DEFAULT 'null'::jsonb NOT NULL
);

CREATE TABLE public.merchant_handoffs (
    digest text NOT NULL,
    user_id text NOT NULL,
    tenant text NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    session_digest text
);

CREATE TABLE public.merchant_identities (
    issuer text NOT NULL,
    subject text NOT NULL,
    user_id text NOT NULL
);

CREATE TABLE public.merchant_mfa (
    user_id text NOT NULL,
    ciphertext text,
    enabled boolean DEFAULT false NOT NULL,
    pending_ciphertext text,
    pending_expires_at timestamp with time zone,
    revision bigint DEFAULT 0 NOT NULL,
    last_step bigint DEFAULT '-1'::integer NOT NULL,
    recovery_digests jsonb DEFAULT '[]'::jsonb NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT merchant_mfa_check CHECK (((NOT enabled) OR (ciphertext IS NOT NULL)))
);

CREATE TABLE public.merchant_mfa_challenges (
    digest text NOT NULL,
    user_id text NOT NULL,
    tenant text NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    expires_at timestamp with time zone DEFAULT (now() + '00:05:00'::interval) NOT NULL
);

CREATE TABLE public.merchant_users (
    id text NOT NULL,
    email text NOT NULL,
    name text NOT NULL,
    password_hash text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    password_initialized boolean DEFAULT true NOT NULL,
    email_verified_at timestamp with time zone,
    credential_revision bigint DEFAULT 0 NOT NULL
);

CREATE TABLE public.observed_pairs (
    tenant text NOT NULL,
    left_id text NOT NULL,
    right_id text NOT NULL,
    orders bigint NOT NULL,
    last_event bigint NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT observed_pairs_check CHECK ((left_id < right_id))
);

CREATE TABLE public.order_activity (
    id bigint NOT NULL,
    tenant text NOT NULL,
    order_id text NOT NULL,
    actor text NOT NULL,
    kind text NOT NULL,
    data jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.order_downloads (
    tenant text NOT NULL,
    order_id text NOT NULL,
    asset_id text NOT NULL
);

CREATE TABLE public.order_inventory_reservations (
    tenant text NOT NULL,
    order_id text NOT NULL,
    product_id text NOT NULL,
    quantity integer NOT NULL,
    released_at timestamp with time zone,
    CONSTRAINT order_inventory_reservations_quantity_check CHECK ((quantity > 0))
);

CREATE TABLE public.order_receipts (
    id text NOT NULL,
    tenant text NOT NULL,
    order_id text NOT NULL,
    kind text NOT NULL,
    number text NOT NULL,
    locale text NOT NULL,
    snapshot jsonb NOT NULL,
    request_key text NOT NULL,
    fingerprint text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    content bytea,
    source_app text,
    customer_visible boolean DEFAULT true NOT NULL,
    CONSTRAINT order_document_size CHECK (((content IS NULL) OR ((octet_length(content) >= 5) AND (octet_length(content) <= 2097152))))
);

CREATE TABLE public.order_state_machines (
    tenant text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL
);

CREATE TABLE public.order_transition_requests (
    tenant text NOT NULL,
    request_key text NOT NULL,
    order_id text NOT NULL,
    fingerprint text NOT NULL,
    response jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.orders (
    id text NOT NULL,
    tenant text NOT NULL,
    cart_id text NOT NULL,
    idempotency_key text NOT NULL,
    fingerprint text NOT NULL,
    data jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.outbox (
    id bigint NOT NULL,
    tenant text NOT NULL,
    kind text NOT NULL,
    data jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    delivered_at timestamp with time zone,
    attempts integer DEFAULT 0 NOT NULL,
    available_at timestamp with time zone DEFAULT now() NOT NULL,
    dead_letter_at timestamp with time zone,
    error_code text,
    payload_retired_at timestamp with time zone,
    subscriber_mask smallint DEFAULT 0 NOT NULL,
    dispatch_not_before timestamp with time zone DEFAULT '-infinity'::timestamp with time zone NOT NULL,
    dispatch_requires_retry boolean DEFAULT false NOT NULL,
    trace_id text DEFAULT COALESCE(NULLIF(current_setting('veyndo.trace_id'::text, true), ''::text), replace((gen_random_uuid())::text, '-'::text, ''::text)) NOT NULL,
    CONSTRAINT outbox_attempts_check CHECK ((attempts >= 0)),
    CONSTRAINT outbox_subscriber_mask_check CHECK (((subscriber_mask >= 0) AND (subscriber_mask <= 15))),
    CONSTRAINT outbox_trace_id_check CHECK ((trace_id ~ '^[a-f0-9]{32}$'::text))
);

CREATE TABLE public.outbox_checkpoints (
    tenant text NOT NULL,
    subscriber text NOT NULL,
    last_event_id bigint DEFAULT 0 NOT NULL,
    served_order bigint DEFAULT 0 NOT NULL,
    CONSTRAINT outbox_checkpoints_last_event_id_check CHECK ((last_event_id >= 0)),
    CONSTRAINT outbox_checkpoints_served_order_check CHECK ((served_order >= 0)),
    CONSTRAINT outbox_checkpoints_subscriber_check CHECK ((subscriber = ANY (ARRAY['cognition'::text, 'apps'::text, 'flows'::text, 'audit'::text])))
);

CREATE TABLE public.outbox_subscriber_failures (
    tenant text NOT NULL,
    event_id bigint NOT NULL,
    subscriber text NOT NULL,
    attempts integer DEFAULT 1 NOT NULL,
    available_at timestamp with time zone DEFAULT now() NOT NULL,
    dead_letter_at timestamp with time zone,
    CONSTRAINT outbox_subscriber_failures_attempts_check CHECK ((attempts >= 0)),
    CONSTRAINT outbox_subscriber_failures_subscriber_check CHECK ((subscriber = ANY (ARRAY['cognition'::text, 'apps'::text, 'flows'::text, 'audit'::text])))
);

CREATE TABLE public.outbox_tenant_dispatch (
    tenant text NOT NULL,
    served_order bigint DEFAULT 0 NOT NULL,
    CONSTRAINT outbox_tenant_dispatch_served_order_check CHECK ((served_order >= 0))
);

CREATE TABLE public.pair_evidence (
    tenant text NOT NULL,
    left_id text NOT NULL,
    right_id text NOT NULL,
    order_id text NOT NULL,
    event_id bigint NOT NULL,
    simulated boolean NOT NULL
);

CREATE TABLE public.payment_attempts (
    id text NOT NULL,
    tenant text NOT NULL,
    order_id text NOT NULL,
    provider text NOT NULL,
    adapter_version text NOT NULL,
    amount_minor bigint NOT NULL,
    currency text NOT NULL,
    state text DEFAULT 'pending'::text NOT NULL,
    provider_order text,
    approval_url text,
    capture_id text,
    refunded_minor bigint DEFAULT 0 NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    expires_at timestamp with time zone DEFAULT (now() + '00:15:00'::interval) NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    environment text DEFAULT 'sandbox'::text NOT NULL,
    bn_code text NOT NULL,
    provider_context jsonb DEFAULT '{}'::jsonb NOT NULL,
    currency_scale smallint DEFAULT 2 NOT NULL,
    capture_confirmed_at timestamp with time zone,
    CONSTRAINT payment_attempts_amount_minor_check CHECK ((amount_minor > 0)),
    CONSTRAINT payment_attempts_currency_check CHECK ((currency ~ '^[A-Z]{3}$'::text)),
    CONSTRAINT payment_attempts_currency_scale_check CHECK ((currency_scale =
CASE
    WHEN (currency = ANY (ARRAY['JPY'::text, 'KRW'::text, 'CLP'::text])) THEN 0
    WHEN (currency = ANY (ARRAY['BHD'::text, 'KWD'::text, 'OMR'::text, 'JOD'::text, 'TND'::text])) THEN 3
    ELSE 2
END))
);

CREATE TABLE public.payment_inbox (
    tenant text NOT NULL,
    provider text NOT NULL,
    event_id text NOT NULL,
    data jsonb NOT NULL,
    received_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.payment_instruments (
    tenant text NOT NULL,
    id text NOT NULL,
    customer_id text NOT NULL,
    provider text NOT NULL,
    adapter_version text NOT NULL,
    account_ref text NOT NULL,
    environment text NOT NULL,
    method text NOT NULL,
    reference text NOT NULL,
    attempt_id text NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    consent_version text NOT NULL,
    state text DEFAULT 'active'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    revoked_at timestamp with time zone,
    CONSTRAINT payment_instruments_check CHECK ((((length(reference) >= 1) AND (length(reference) <= 200)) AND ((length(consent_version) >= 1) AND (length(consent_version) <= 50)))),
    CONSTRAINT payment_instruments_environment_check CHECK ((environment = ANY (ARRAY['sandbox'::text, 'live'::text, 'simulation'::text, 'contract-fixture'::text]))),
    CONSTRAINT payment_instruments_state_check CHECK ((state = ANY (ARRAY['active'::text, 'revoked'::text])))
);

CREATE TABLE public.payment_jobs (
    id text NOT NULL,
    tenant text NOT NULL,
    attempt_id text NOT NULL,
    operation text NOT NULL,
    request jsonb DEFAULT '{}'::jsonb NOT NULL,
    fingerprint text NOT NULL,
    state text DEFAULT 'queued'::text NOT NULL,
    lease_until timestamp with time zone,
    attempts integer DEFAULT 0 NOT NULL,
    available_at timestamp with time zone DEFAULT now() NOT NULL,
    response jsonb,
    error text,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.payment_provider_accounts (
    tenant text NOT NULL,
    app text NOT NULL,
    channel text DEFAULT 'default'::text NOT NULL,
    account_ref text NOT NULL,
    environment text NOT NULL,
    ready boolean DEFAULT false NOT NULL,
    data jsonb DEFAULT '{}'::jsonb NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT payment_provider_accounts_environment_check CHECK ((environment = ANY (ARRAY['sandbox'::text, 'live'::text, 'simulation'::text, 'contract-fixture'::text])))
);

CREATE TABLE public.payment_receipt_claims (
    provider text NOT NULL,
    environment text NOT NULL,
    account_ref text NOT NULL,
    kind text NOT NULL,
    receipt_ref text NOT NULL,
    tenant text NOT NULL,
    attempt_id text NOT NULL,
    job_key text NOT NULL
);

CREATE TABLE public.payment_refunds (
    tenant text NOT NULL,
    job_id text NOT NULL,
    attempt_id text NOT NULL,
    provider_id text NOT NULL,
    status text NOT NULL,
    amount_minor bigint NOT NULL
);

CREATE TABLE public.platform_ai (
    id boolean DEFAULT true NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    data jsonb DEFAULT '{}'::jsonb NOT NULL,
    secrets jsonb DEFAULT '{}'::jsonb NOT NULL,
    updated_by text,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT platform_ai_id_check CHECK (id)
);

CREATE TABLE public.platform_audit (
    id bigint NOT NULL,
    actor text,
    action text NOT NULL,
    tenant text,
    data jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.platform_operators (
    user_id text NOT NULL,
    active boolean DEFAULT true NOT NULL,
    granted_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.policy (
    tenant text NOT NULL,
    variant text NOT NULL,
    views bigint DEFAULT 0 NOT NULL,
    purchases bigint DEFAULT 0 NOT NULL
);

CREATE TABLE public.privacy_consent_log (
    id bigint NOT NULL,
    tenant text NOT NULL,
    cart_id text NOT NULL,
    channel_id text NOT NULL,
    data jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.privacy_consents (
    tenant text NOT NULL,
    cart_id text NOT NULL,
    channel_id text NOT NULL,
    policy_version text NOT NULL,
    data jsonb NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    source_cart_id text,
    CONSTRAINT privacy_consents_source_distinct CHECK (((source_cart_id IS NULL) OR (source_cart_id <> cart_id)))
);

CREATE TABLE public.private_preferences (
    tenant text NOT NULL,
    cart_id text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.product_assets (
    tenant text NOT NULL,
    id text NOT NULL,
    product_id text NOT NULL,
    title jsonb NOT NULL,
    filename text NOT NULL,
    mime text NOT NULL,
    kind text NOT NULL,
    public boolean DEFAULT false NOT NULL,
    content bytea NOT NULL,
    digest text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT product_assets_kind_check CHECK ((kind = ANY (ARRAY['attachment'::text, 'download'::text])))
);

CREATE TABLE public.product_categories (
    tenant text NOT NULL,
    product_id text NOT NULL,
    category_id text NOT NULL
);

CREATE TABLE public.product_channel_visibility (
    tenant text NOT NULL,
    product_id text NOT NULL,
    channel_id text NOT NULL,
    visible boolean NOT NULL
);

CREATE TABLE public.product_reviews (
    id text NOT NULL,
    tenant text NOT NULL,
    product_id text NOT NULL,
    session text NOT NULL,
    author text NOT NULL,
    rating integer NOT NULL,
    title text NOT NULL,
    content text NOT NULL,
    approved boolean DEFAULT false NOT NULL,
    verified boolean DEFAULT false NOT NULL,
    demo boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    source_import jsonb,
    CONSTRAINT product_reviews_author_check CHECK (((length(author) >= 1) AND (length(author) <= 80))),
    CONSTRAINT product_reviews_content_check CHECK (((length(content) >= 1) AND (length(content) <= 3000))),
    CONSTRAINT product_reviews_rating_check CHECK (((rating >= 1) AND (rating <= 5))),
    CONSTRAINT product_reviews_title_check CHECK (((length(title) >= 1) AND (length(title) <= 120))),
    CONSTRAINT review_source_provenance CHECK (((source_import IS NULL) OR ((jsonb_typeof(source_import) = 'object'::text) AND (octet_length((source_import)::text) <= 8192) AND (NOT verified))))
);

CREATE TABLE public.product_translations (
    tenant text NOT NULL,
    product_id text NOT NULL,
    language_id text NOT NULL,
    name text,
    description text
);

CREATE TABLE public.products (
    tenant text NOT NULL,
    id text NOT NULL,
    name text NOT NULL,
    category text NOT NULL,
    description text NOT NULL,
    price double precision NOT NULL,
    tax_rate double precision NOT NULL,
    stock integer NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    list_price double precision,
    regulation_price double precision,
    reference_price jsonb,
    advanced_prices jsonb DEFAULT '[]'::jsonb NOT NULL,
    min_purchase integer DEFAULT 1 NOT NULL,
    purchase_steps integer DEFAULT 1 NOT NULL,
    max_purchase integer,
    parent_id text,
    options jsonb DEFAULT '{}'::jsonb NOT NULL,
    media jsonb DEFAULT '[]'::jsonb NOT NULL,
    properties jsonb DEFAULT '{}'::jsonb NOT NULL,
    delivery_days integer DEFAULT 2 NOT NULL,
    extra jsonb DEFAULT '{}'::jsonb NOT NULL,
    active boolean DEFAULT true NOT NULL,
    product_number text,
    created_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    content_revision bigint DEFAULT 1 NOT NULL,
    CONSTRAINT products_content_revision_check CHECK ((content_revision >= 0)),
    CONSTRAINT products_delivery_days_check CHECK (((delivery_days >= 0) AND (delivery_days <= 365))),
    CONSTRAINT products_max_purchase_check CHECK ((max_purchase > 0)),
    CONSTRAINT products_min_purchase_check CHECK ((min_purchase > 0)),
    CONSTRAINT products_price_check CHECK ((price >= (0)::double precision)),
    CONSTRAINT products_purchase_steps_check CHECK ((purchase_steps > 0)),
    CONSTRAINT products_stock_check CHECK ((stock >= 0))
);

CREATE TABLE public.projections (
    event_id bigint NOT NULL,
    tenant text NOT NULL,
    kind text NOT NULL,
    data jsonb NOT NULL,
    projected_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.promotion_uses (
    tenant text NOT NULL,
    promotion text NOT NULL,
    order_id text NOT NULL
);

CREATE TABLE public.read_context_versions (
    namespace text NOT NULL,
    version uuid DEFAULT gen_random_uuid() NOT NULL
);

CREATE TABLE public.receipt_counters (
    tenant text NOT NULL,
    kind text NOT NULL,
    next_number bigint DEFAULT 1 NOT NULL
);

CREATE TABLE public.receipt_settings (
    tenant text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL
);

CREATE TABLE public.resource_leases (
    id text NOT NULL,
    tenant text NOT NULL,
    class text NOT NULL,
    slots integer DEFAULT 1 NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT resource_leases_slots_check CHECK ((slots > 0))
);

CREATE TABLE public.sales_channels (
    tenant text NOT NULL,
    id text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL
);

CREATE TABLE public.semantic_products (
    tenant text NOT NULL,
    product_id text NOT NULL,
    revision bigint NOT NULL,
    embedding real[] NOT NULL,
    embedding_model text NOT NULL,
    content_hash text NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    vector_digest text,
    CONSTRAINT cognitive_product_geometry CHECK (((cardinality(embedding) >= 1) AND (cardinality(embedding) <= 8192)))
);

CREATE TABLE public.session_signal_events (
    tenant text NOT NULL,
    id text NOT NULL,
    session text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    purpose text DEFAULT 'personalization'::text NOT NULL,
    kind text DEFAULT 'signal'::text NOT NULL,
    cart_id text,
    channel_id text,
    product_id text,
    customer_id text,
    dimensions jsonb DEFAULT '{}'::jsonb NOT NULL,
    expires_at timestamp with time zone DEFAULT (now() + '30 days'::interval) NOT NULL,
    CONSTRAINT behavior_dimensions_bound CHECK ((octet_length((dimensions)::text) <= 2048)),
    CONSTRAINT session_signal_events_purpose_check CHECK ((purpose = ANY (ARRAY['personalization'::text, 'analytics'::text])))
);

CREATE TABLE public.session_signals (
    tenant text NOT NULL,
    session text NOT NULL,
    product_id text NOT NULL,
    views integer DEFAULT 0 NOT NULL,
    cart_adds integer DEFAULT 0 NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.shop_environments (
    tenant text NOT NULL,
    live_tenant text NOT NULL,
    name text NOT NULL,
    baseline jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    preview_owner text,
    preview_until timestamp with time zone,
    CONSTRAINT preview_owner_expiry CHECK (((preview_owner IS NULL) = (preview_until IS NULL))),
    CONSTRAINT shop_environments_check CHECK ((tenant <> live_tenant))
);

CREATE TABLE public.shop_releases (
    id text NOT NULL,
    live_tenant text NOT NULL,
    environment text NOT NULL,
    selections jsonb NOT NULL,
    actor text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.studio_workspace_versions (
    tenant text NOT NULL,
    user_id text NOT NULL,
    revision bigint NOT NULL,
    data jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.studio_workspaces (
    tenant text NOT NULL,
    user_id text NOT NULL,
    revision bigint NOT NULL,
    data jsonb NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT studio_workspaces_data_check CHECK ((pg_column_size(data) <= 32768)),
    CONSTRAINT studio_workspaces_revision_check CHECK ((revision > 0))
);

CREATE TABLE public.tasks (
    id text NOT NULL,
    tenant text NOT NULL,
    proposal jsonb NOT NULL,
    applied boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    applied_at timestamp with time zone,
    run_state text,
    run_request jsonb,
    run_actor text,
    run_key text,
    run_fingerprint text,
    run_scopes jsonb,
    run_credential text,
    run_personal boolean DEFAULT false NOT NULL,
    run_checkpoint jsonb DEFAULT '{}'::jsonb NOT NULL,
    run_pending text,
    run_lease text,
    run_until timestamp with time zone,
    run_revision bigint DEFAULT 1 NOT NULL,
    run_error jsonb,
    run_updated timestamp with time zone DEFAULT now() NOT NULL,
    conversation_id text,
    run_trace text DEFAULT COALESCE(NULLIF(current_setting('veyndo.trace_id'::text, true), ''::text), replace((gen_random_uuid())::text, '-'::text, ''::text)) NOT NULL,
    run_after timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT tasks_run_checkpoint_limit CHECK ((octet_length((run_checkpoint)::text) <= 1048576)),
    CONSTRAINT tasks_run_request_limit CHECK ((octet_length((run_request)::text) <= 32768)),
    CONSTRAINT tasks_run_state_check CHECK ((run_state = ANY (ARRAY['queued'::text, 'running'::text, 'ready'::text, 'failed'::text, 'uncertain'::text, 'cancelled'::text]))),
    CONSTRAINT tasks_run_trace_check CHECK ((run_trace ~ '^[a-f0-9]{32}$'::text))
);

CREATE TABLE public.tenant_ai_usage (
    tenant text NOT NULL,
    day date NOT NULL,
    attempts bigint NOT NULL,
    CONSTRAINT tenant_ai_usage_attempts_check CHECK ((attempts > 0))
);

CREATE TABLE public.tenant_resource_limits (
    tenant text NOT NULL,
    daily_ai bigint NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    CONSTRAINT tenant_resource_limits_daily_ai_check CHECK (((daily_ai >= 1) AND (daily_ai <= 1000000))),
    CONSTRAINT tenant_resource_limits_revision_check CHECK ((revision > 0))
);

CREATE TABLE public.tenants (
    id text NOT NULL,
    name text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    status text DEFAULT 'active'::text NOT NULL,
    status_revision bigint DEFAULT 1 NOT NULL,
    status_updated_at timestamp with time zone DEFAULT now() NOT NULL,
    agents_stopped boolean DEFAULT false NOT NULL,
    agents_revision bigint DEFAULT 1 NOT NULL,
    CONSTRAINT tenants_agents_revision_check CHECK ((agents_revision > 0)),
    CONSTRAINT tenants_id_check CHECK ((id ~ '^[a-z0-9][a-z0-9-]{1,47}$'::text)),
    CONSTRAINT tenants_status_check CHECK ((status = ANY (ARRAY['active'::text, 'paused'::text, 'archived'::text])))
);

CREATE TABLE public.translation_items (
    tenant text NOT NULL,
    job_id text NOT NULL,
    product_id text NOT NULL,
    revision bigint NOT NULL,
    source jsonb NOT NULL,
    result jsonb NOT NULL,
    status text DEFAULT 'ready'::text NOT NULL,
    evidence jsonb,
    CONSTRAINT translation_evidence_budget CHECK (((evidence IS NULL) OR ((jsonb_typeof(evidence) = 'object'::text) AND (octet_length((evidence)::text) <= 8192))))
);

CREATE TABLE public.translation_jobs (
    tenant text NOT NULL,
    id text NOT NULL,
    source_locale text NOT NULL,
    target_locale text NOT NULL,
    choice jsonb NOT NULL,
    overwrite boolean DEFAULT false NOT NULL,
    status text DEFAULT 'queued'::text NOT NULL,
    cursor text DEFAULT ''::text NOT NULL,
    highwater text DEFAULT ''::text NOT NULL,
    total bigint NOT NULL,
    processed bigint DEFAULT 0 NOT NULL,
    lease text,
    lease_until timestamp with time zone,
    error text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    product_ids text[],
    run_task text,
    error_code text,
    CONSTRAINT translation_jobs_error_code_check CHECK (((error_code IS NULL) OR (error_code = 'call_interrupted'::text))),
    CONSTRAINT translation_selection_budget CHECK (((product_ids IS NULL) OR (((cardinality(product_ids) >= 1) AND (cardinality(product_ids) <= 1000)) AND (array_position(product_ids, NULL::text) IS NULL))))
);

CREATE TABLE public.user_invites (
    id text NOT NULL,
    digest text NOT NULL,
    tenant text NOT NULL,
    email text NOT NULL,
    role text NOT NULL,
    created_by text NOT NULL,
    expires_at timestamp with time zone DEFAULT (now() + '24:00:00'::interval) NOT NULL,
    redeemed_at timestamp with time zone
);

CREATE TABLE public.user_sessions (
    digest text NOT NULL,
    user_id text NOT NULL,
    default_tenant text NOT NULL,
    expires_at timestamp with time zone DEFAULT (now() + '12:00:00'::interval) NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.vector_index_queue (
    tenant text NOT NULL,
    kind text NOT NULL,
    object_id text NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    lease text,
    lease_until timestamp with time zone,
    attempts integer DEFAULT 0 NOT NULL,
    available_at timestamp with time zone DEFAULT now() NOT NULL,
    error_code text
);

CREATE TABLE public.workspace_roles (
    tenant text NOT NULL,
    id text NOT NULL,
    data jsonb NOT NULL,
    builtin boolean DEFAULT false NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    CONSTRAINT workspace_roles_id_check CHECK ((id ~ '^[a-z][a-z0-9_]{0,31}$'::text)),
    CONSTRAINT workspace_roles_revision_check CHECK ((revision > 0))
);

CREATE TABLE veyndo_authority.configuration (
    id boolean DEFAULT true NOT NULL,
    enforced boolean DEFAULT false NOT NULL,
    system_role name,
    migration_role name NOT NULL,
    tenant_login name,
    CONSTRAINT configuration_id_check CHECK (id)
);

CREATE TABLE veyndo_authority.keys (
    id text NOT NULL,
    secret bytea NOT NULL,
    active boolean NOT NULL,
    verify_until timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT keys_secret_check CHECK ((octet_length(secret) = 32))
);

CREATE TRIGGER app_record_prepare BEFORE INSERT OR UPDATE ON public.app_records FOR EACH ROW EXECUTE FUNCTION public.app_record_prepare();

CREATE TRIGGER app_record_project AFTER INSERT OR DELETE OR UPDATE ON public.app_records FOR EACH ROW EXECUTE FUNCTION public.app_record_project();

CREATE TRIGGER cognitive_document_intake AFTER INSERT OR UPDATE ON public.knowledge_chunks FOR EACH ROW EXECUTE FUNCTION public.cognitive_source_changed('document');

CREATE TRIGGER cognitive_product_intake AFTER INSERT OR UPDATE ON public.products FOR EACH ROW EXECUTE FUNCTION public.cognitive_source_changed('product');

CREATE TRIGGER cognitive_relation_write BEFORE INSERT OR DELETE OR UPDATE ON public.knowledge_relations FOR EACH ROW EXECUTE FUNCTION public.cognitive_relation_write();

CREATE TRIGGER cognitive_status_errors AFTER INSERT OR DELETE OR UPDATE OF error_code, tenant ON public.embedding_jobs FOR EACH ROW EXECUTE FUNCTION public.cognitive_error_delta();

CREATE TRIGGER cognitive_status_index AFTER INSERT OR DELETE ON public.semantic_products FOR EACH ROW EXECUTE FUNCTION public.cognitive_status_delta();

CREATE TRIGGER cognitive_status_pending AFTER INSERT OR DELETE ON public.embedding_jobs FOR EACH ROW EXECUTE FUNCTION public.cognitive_status_delta();

CREATE TRIGGER cognitive_vector_digest BEFORE INSERT OR UPDATE OF embedding ON public.semantic_products FOR EACH ROW EXECUTE FUNCTION public.cognitive_vector_digest();

CREATE TRIGGER commerce_overrides_cache_version BEFORE INSERT OR UPDATE ON public.commerce_overrides FOR EACH ROW EXECUTE FUNCTION public.stamp_read_context();

CREATE TRIGGER commerce_settings_cache_version BEFORE INSERT OR UPDATE ON public.commerce_settings FOR EACH ROW EXECUTE FUNCTION public.stamp_read_context();

CREATE TRIGGER document_index_outbox AFTER INSERT OR DELETE OR UPDATE ON public.knowledge_chunks FOR EACH ROW EXECUTE FUNCTION public.queue_vector_index('document');

CREATE TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.b2b_approvals FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bApproval', 'id');

CREATE TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.b2b_catalog FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bUnit', 'unit_id');

CREATE TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.b2b_companies FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bCompany', 'id');

CREATE TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.b2b_lists FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bList', 'id');

CREATE TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.b2b_members FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bUnit', 'unit_id');

CREATE TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.b2b_quotes FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bQuote', 'id');

CREATE TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.b2b_units FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bUnit', 'id');

CREATE TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.workspace_roles FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('role', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.b2b_approvals FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bApproval', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.b2b_catalog FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bUnit', 'unit_id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.b2b_companies FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bCompany', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.b2b_lists FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bList', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.b2b_members FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bUnit', 'unit_id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.b2b_quotes FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bQuote', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.b2b_units FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bUnit', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.categories FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('category', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.commerce_flows FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('flow', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.commerce_overrides FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('checkoutChannel', 'channel_id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.commerce_promotions FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('promotion', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.commerce_rules FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('rule', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.commerce_settings FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('settings', 'tenant');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.company_overrides FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('companyChannel', 'channel_id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.customer_addresses FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('customer', 'email');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.customers FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('customer', 'email');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.knowledge_documents FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('source', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.orders FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('order', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.product_categories FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('product', 'product_id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.product_channel_visibility FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('product', 'product_id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.product_translations FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('product', 'product_id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.products FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('product', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.receipt_settings FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('company', 'tenant');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.sales_channels FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('channel', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.workspace_roles FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('role', 'id');

CREATE TRIGGER intelligence_capture_clock BEFORE INSERT OR UPDATE ON public.payment_attempts FOR EACH ROW EXECUTE FUNCTION public.intelligence_capture_clock();

CREATE TRIGGER knowledge_chunk_words_changed AFTER INSERT OR UPDATE OF text ON public.knowledge_chunks FOR EACH ROW EXECUTE FUNCTION public.knowledge_chunk_words();

CREATE TRIGGER knowledge_product_words_changed AFTER INSERT OR UPDATE OF name, description ON public.products FOR EACH ROW EXECUTE FUNCTION public.knowledge_product_words();

CREATE TRIGGER languages_cache_version AFTER DELETE OR UPDATE OR TRUNCATE ON public.languages FOR EACH STATEMENT EXECUTE FUNCTION public.stamp_language_registry();

CREATE TRIGGER languages_insert_cache_version AFTER INSERT ON public.languages REFERENCING NEW TABLE AS changed_languages FOR EACH STATEMENT EXECUTE FUNCTION public.stamp_new_languages();

CREATE TRIGGER order_document_quota AFTER INSERT OR DELETE OR UPDATE ON public.order_receipts FOR EACH ROW EXECUTE FUNCTION public.order_document_accounting();

CREATE TRIGGER outbox_cache_notification AFTER INSERT ON public.outbox FOR EACH ROW EXECUTE FUNCTION public.notify_outbox_cache();

CREATE TRIGGER overrides_cache_event AFTER INSERT OR UPDATE ON public.commerce_overrides FOR EACH ROW EXECUTE FUNCTION public.emit_read_cache_event();

CREATE TRIGGER prepare_outbox_delivery BEFORE INSERT ON public.outbox FOR EACH ROW EXECUTE FUNCTION public.prepare_outbox_delivery();

CREATE TRIGGER product_content_clock BEFORE UPDATE ON public.products FOR EACH ROW EXECUTE FUNCTION public.product_content_clock();

CREATE TRIGGER register_outbox_subscribers AFTER INSERT ON public.tenants FOR EACH ROW EXECUTE FUNCTION public.register_outbox_subscribers();

CREATE TRIGGER register_outbox_tenant AFTER INSERT ON public.outbox FOR EACH ROW EXECUTE FUNCTION public.register_outbox_tenant();

CREATE TRIGGER semantic_index_outbox AFTER INSERT OR DELETE OR UPDATE ON public.semantic_products FOR EACH ROW EXECUTE FUNCTION public.queue_vector_index('product');

CREATE TRIGGER settings_cache_event AFTER INSERT OR UPDATE ON public.commerce_settings FOR EACH ROW EXECUTE FUNCTION public.emit_read_cache_event();

CREATE TRIGGER tasks_run_progress AFTER INSERT OR UPDATE OF run_state ON public.tasks FOR EACH ROW EXECUTE FUNCTION public.emit_agent_run_state();

CREATE TRIGGER work_ready AFTER INSERT OR UPDATE ON public.app_deliveries FOR EACH ROW EXECUTE FUNCTION public.notify_work_ready();

CREATE TRIGGER work_ready AFTER INSERT OR UPDATE ON public.currency_price_jobs FOR EACH ROW EXECUTE FUNCTION public.notify_work_ready();

CREATE TRIGGER work_ready AFTER INSERT OR UPDATE ON public.flow_jobs FOR EACH ROW EXECUTE FUNCTION public.notify_work_ready();

CREATE TRIGGER work_ready AFTER INSERT OR UPDATE ON public.media_jobs FOR EACH ROW EXECUTE FUNCTION public.notify_work_ready();

CREATE TRIGGER work_ready AFTER INSERT OR UPDATE ON public.outbox FOR EACH ROW EXECUTE FUNCTION public.notify_work_ready();

CREATE TRIGGER work_ready AFTER INSERT OR UPDATE ON public.payment_jobs FOR EACH ROW EXECUTE FUNCTION public.notify_work_ready();

CREATE TRIGGER work_ready AFTER INSERT OR UPDATE ON public.translation_jobs FOR EACH ROW EXECUTE FUNCTION public.notify_work_ready();

CREATE TRIGGER workspace_role_defaults AFTER INSERT ON public.tenants FOR EACH ROW EXECUTE FUNCTION public.initialize_workspace_roles();

CREATE UNIQUE INDEX app_preview_owner ON public.shop_environments USING btree (live_tenant, preview_owner) WHERE (preview_owner IS NOT NULL);

CREATE UNIQUE INDEX app_record_unique ON public.app_record_indexes USING btree (tenant, app, entity, field, value_hash) WHERE is_unique;

CREATE UNIQUE INDEX intelligence_one_experiment ON public.intelligence_experiments USING btree (tenant, channel) WHERE (state = 'running'::text);

CREATE UNIQUE INDEX product_number_unique ON public.products USING btree (tenant, product_number);

CREATE UNIQUE INDEX scope_active_key ON veyndo_authority.keys USING btree (active) WHERE active;

CREATE UNIQUE INDEX tasks_one_conversation_run ON public.tasks USING btree (tenant, conversation_id) WHERE ((conversation_id IS NOT NULL) AND (run_state = ANY (ARRAY['queued'::text, 'running'::text, 'uncertain'::text])));

CREATE UNIQUE INDEX tasks_run_request_key ON public.tasks USING btree (tenant, run_actor, COALESCE((run_request ->> 'agentId'::text), ''::text), run_key) WHERE (run_key IS NOT NULL);

CREATE VIEW public.membership_grants WITH (security_invoker='true') AS
 SELECT m.user_id,
    m.tenant,
    m.role,
    (m.active AND COALESCE(((r.data ->> 'active'::text))::boolean, false)) AS active,
        CASE
            WHEN (m.role = 'owner'::text) THEN 'null'::jsonb
            ELSE COALESCE(NULLIF(m.permissions, 'null'::jsonb), (r.data -> 'permissions'::text), '[]'::jsonb)
        END AS permissions
   FROM (public.memberships m
     JOIN public.workspace_roles r ON (((r.tenant = m.tenant) AND (r.id = m.role))));

REVOKE ALL ON FUNCTION public.app_record_prepare() FROM PUBLIC;

REVOKE ALL ON FUNCTION public.app_record_project() FROM PUBLIC;

-- rows
COPY public.ai_price_budget (tenant, day, product_id, baseline_minor, currency, task_id, actor) FROM stdin;


COPY public.app_action_log (id, tenant, app, action, actor, status, duration_ms, input_digest, created_at) FROM stdin;


COPY public.app_deliveries (tenant, app, event_id, state, attempts, lease_until, available_at, error, lease_token, last_attempt_at) FROM stdin;


COPY public.app_evidence (tenant, app, source_id, kind, title, body, source_url, metadata, digest, updated_at) FROM stdin;


COPY public.app_export_cursors (tenant, app, cursor, last_poll) FROM stdin;


COPY public.app_jobs (tenant, app, id, action, actor, permission, package_digest, request_key, input_digest, input, result, status, progress, message, lease, lease_until, started_at, created_at, updated_at, revision) FROM stdin;


COPY public.app_models (tenant, app, entity, version, definition) FROM stdin;


COPY public.app_packages (tenant, id, version, manifest, digest, active, revision, installed_at) FROM stdin;


COPY public.app_record_indexes (tenant, app, entity, field, record_id, value, is_unique) FROM stdin;


COPY public.app_record_references (tenant, app, entity, record_id, field, "position", target_entity, target_id, product_id, customer_id, order_id, channel_id, company_id, unit_id, asset_id) FROM stdin;


COPY public.app_record_versions (tenant, app, entity, record_id, revision, fields, read_scopes, actor, source, created_at) FROM stdin;


COPY public.app_records (tenant, app, entity, id, revision, version, fields, storage_rows, storage_bytes) FROM stdin;


COPY public.app_schedule_runs (tenant, app, schedule, due, event_id) FROM stdin;


COPY public.app_schedules (tenant, app, id, definition, next_run) FROM stdin;


COPY public.app_schema_history (tenant, app, to_version, from_version, old_manifest, records, created_at) FROM stdin;


COPY public.app_service_secrets (tenant, app, kind, ciphertext, package_digest, revision, actor, rotated_at) FROM stdin;


COPY public.app_storage_usage (tenant, app, rows, bytes, updated_at) FROM stdin;


COPY public.app_surface_grants (tenant, token_hash, app, surface, actor_hash, package_digest, actions, context, expires_at) FROM stdin;


COPY public.app_versions (tenant, app, version, digest, manifest, installed_at) FROM stdin;


COPY public.app_webhook_receipts (tenant, app, webhook, request_key, digest, event_id) FROM stdin;


COPY public.auth_attempt_buckets (key, window_at, attempts, blocked_until, updated_at) FROM stdin;


COPY public.b2b_approvals (tenant, id, unit_id, buyer_id, cart_id, fingerprint, data, state, revision, created_at) FROM stdin;


COPY public.b2b_budget_allocations (tenant, order_id, unit_id, period_start, currency, amount_minor, released) FROM stdin;


COPY public.b2b_catalog (tenant, unit_id, product_id) FROM stdin;


COPY public.b2b_companies (tenant, id, data, revision, created_at) FROM stdin;


COPY public.b2b_lists (tenant, id, unit_id, owner_id, data, revision) FROM stdin;


COPY public.b2b_members (tenant, unit_id, customer_id, data, revision) FROM stdin;


COPY public.b2b_quotes (tenant, id, unit_id, buyer_id, cart_id, state, data, revision, created_at) FROM stdin;


COPY public.b2b_units (tenant, id, company_id, parent_id, data, revision) FROM stdin;


COPY public.carts (id, tenant, token, data, revision, status, created_at) FROM stdin;


COPY public.categories (tenant, id, parent_id, "position", data, revision) FROM stdin;


COPY public.channel_metrics (tenant, channel, calls, failures, last_seen, total_ms, max_ms, timed_calls, response_counts) FROM stdin;


COPY public.channel_previews (digest, tenant, channel, session_digest, channel_revision, alias, redeemed, expires_at) FROM stdin;


COPY public.chat_messages (id, tenant, conversation_id, role, content, data, created_at) FROM stdin;


COPY public.checkout_handoffs (digest, tenant, cart_id, expires_at, recovery_digest, recovery_origin, recovery_expires_at, consumed_at) FROM stdin;


COPY public.commerce_customer_groups (tenant, id, data, revision) FROM stdin;


COPY public.commerce_flows (tenant, id, data, revision) FROM stdin;


COPY public.commerce_overrides (tenant, channel_id, data, revision, cache_version) FROM stdin;


COPY public.commerce_promotions (tenant, id, data, revision) FROM stdin;


COPY public.commerce_rules (tenant, id, name, condition, active, revision) FROM stdin;


COPY public.commerce_settings (tenant, data, revision, cache_version) FROM stdin;


COPY public.company_logos (tenant, id, content, digest, mime, width, height, created_at) FROM stdin;


COPY public.company_overrides (tenant, channel_id, data, revision) FROM stdin;


COPY public.connector_changes (seq, tenant, app, data) FROM stdin;


COPY public.connector_config (tenant, app, data) FROM stdin;


COPY public.connector_jobs (tenant, app, id, fingerprint, state, payload, result, available, attempts, lease_id, lease_until, created_at) FROM stdin;


COPY public.connector_limits (tenant, app, day, enqueued, minute, dispatched, last_started) FROM stdin;


COPY public.connector_oauth (digest, tenant, app, data, expires_at) FROM stdin;


COPY public.connector_sources (tenant, app, id, data, updated_at) FROM stdin;


COPY public.consumer_request_reviews (tenant, request_id, revision, actor, state, note, created_at) FROM stdin;


COPY public.consumer_requests (tenant, id, cart_id, channel_id, kind, data, state, revision, created_at) FROM stdin;


COPY public.conversations (tenant, id, title, created_at, turn_owner, turn_until, owner_id) FROM stdin;


COPY public.currency_price_jobs (tenant, id, data, state, cursor, ceiling, processed, created_at) FROM stdin;


COPY public.customer_addresses (tenant, email, id, revision, data, created_at, updated_at) FROM stdin;


COPY public.customer_sessions (digest, tenant, email, expires_at) FROM stdin;


COPY public.customers (tenant, email, password_hash, company, group_name, profile, active, revision, created_at, id, customer_number, default_billing_address_id, default_shipping_address_id, sales_channel_id, language_id, first_login, last_login, last_payment_method_id, automation) FROM stdin;


COPY public.developer_builds (id, tenant, environment, app, version, digest, manifest, prompt, provider, model, summary, state, created_at, archived, archived_at) FROM stdin;


COPY public.developer_drafts (tenant, actor_key, id, manifest, environment, revision, updated_at) FROM stdin;


COPY public.embedding_generations (tenant, model, phase, cursor, updated_at, model_revision) FROM stdin;


COPY public.embedding_jobs (tenant, kind, object_id, revision, attempts, available_at, lease, lease_until, error_code) FROM stdin;


COPY public.entity_history (id, tenant, entity, entity_id, transaction_id, actor, source, reason, before_state, after_state, created_at, finalized) FROM stdin;


COPY public.experiences (tenant, data, revision) FROM stdin;


COPY public.exposures (tenant, session, variant, propensity, rewarded, created_at) FROM stdin;


COPY public.extensions (tenant, wat, digest, revision) FROM stdin;


COPY public.flow_jobs (id, tenant, flow, event_id, definition, state, lease_until, attempts, result, error, available_at, cursor, execution) FROM stdin;


COPY public.flow_steps (job, node, state, result, error, started_at, finished_at) FROM stdin;


COPY public.hosted_frontends (alias, tenant, channel, origin, created_at, experience_alias, revision, app_id) FROM stdin;


COPY public.identity_inference_daily (email, day, calls) FROM stdin;


COPY public.identity_nonces (digest, expires_at) FROM stdin;


COPY public.integration_keys (id, tenant, user_id, digest, name, permissions, expires_at, created_at, app_id, package_digest, app_permissions, agent_budget, agent_tokens_allocated, agent_cost_allocated) FROM stdin;


COPY public.intelligence_assignments (tenant, experiment_id, cart_id, arm, baseline, created_at) FROM stdin;


COPY public.intelligence_experiments (tenant, id, channel, data, revision, state, withdrawn, actor, started_at, ends_at, created_at, final_report) FROM stdin;


COPY public.inventory_reservations (tenant, attempt_id, product_id, quantity, released) FROM stdin;


COPY public.knowledge_chunk_lexemes (tenant, document_id, "position", lexeme) FROM stdin;


COPY public.knowledge_chunks (tenant, document_id, "position", text, embedding, embedding_model, locale) FROM stdin;


COPY public.knowledge_documents (tenant, id, product_id, title, content_hash, content, visibility, source_type, revision, created_at, kind, locale, translations, archived) FROM stdin;


COPY public.knowledge_hypotheses (tenant, id, kind, evidence, state, revision, updated_at) FROM stdin;


COPY public.knowledge_index_status (tenant, indexed_products, pending_jobs, errors) FROM stdin;
atelier	0	0	{}
workshop	0	0	{}

COPY public.knowledge_nodes (tenant, kind, id, data, revision) FROM stdin;


COPY public.knowledge_product_lexemes (tenant, product_id, lexeme) FROM stdin;


COPY public.knowledge_receipts (tenant, event_id) FROM stdin;


COPY public.knowledge_relation_history (tenant, kind, source_id, target_id, revision, snapshot, recorded_from, recorded_until) FROM stdin;


COPY public.knowledge_relations (tenant, kind, source_id, target_id, data, source_kind, target_kind, state, revision, confidence, valid_from, valid_until, recorded_from, actor) FROM stdin;


COPY public.languages (id, locale, parent_id) FROM stdin;
11111111111111111111111111111111	de-DE	\N
22222222222222222222222222222222	fr-FR	\N
2fbb5fe2e29a4d70aa5854ce7ce3e20b	en-GB	\N
33333333333333333333333333333333	es-ES	\N
44444444444444444444444444444444	de-CH	11111111111111111111111111111111

COPY public.legal_acceptances (tenant, cart_id, data) FROM stdin;


COPY public.media_jobs (tenant, id, product_id, product_revision, request, state, asset_id, error, started_at, created_at) FROM stdin;


COPY public.memberships (user_id, tenant, role, active, permissions) FROM stdin;


COPY public.merchant_handoffs (digest, user_id, tenant, expires_at, session_digest) FROM stdin;


COPY public.merchant_identities (issuer, subject, user_id) FROM stdin;


COPY public.merchant_mfa (user_id, ciphertext, enabled, pending_ciphertext, pending_expires_at, revision, last_step, recovery_digests, updated_at) FROM stdin;


COPY public.merchant_mfa_challenges (digest, user_id, tenant, attempts, expires_at) FROM stdin;


COPY public.merchant_users (id, email, name, password_hash, created_at, password_initialized, email_verified_at, credential_revision) FROM stdin;


COPY public.observed_pairs (tenant, left_id, right_id, orders, last_event, updated_at) FROM stdin;


COPY public.order_activity (id, tenant, order_id, actor, kind, data, created_at) FROM stdin;


COPY public.order_downloads (tenant, order_id, asset_id) FROM stdin;


COPY public.order_inventory_reservations (tenant, order_id, product_id, quantity, released_at) FROM stdin;


COPY public.order_receipts (id, tenant, order_id, kind, number, locale, snapshot, request_key, fingerprint, created_at, content, source_app, customer_visible) FROM stdin;


COPY public.order_state_machines (tenant, data, revision) FROM stdin;


COPY public.order_transition_requests (tenant, request_key, order_id, fingerprint, response, created_at) FROM stdin;


COPY public.orders (id, tenant, cart_id, idempotency_key, fingerprint, data, created_at) FROM stdin;


COPY public.outbox (id, tenant, kind, data, created_at, delivered_at, attempts, available_at, dead_letter_at, error_code, payload_retired_at, subscriber_mask, dispatch_not_before, dispatch_requires_retry, trace_id) FROM stdin;


COPY public.outbox_checkpoints (tenant, subscriber, last_event_id, served_order) FROM stdin;
atelier	apps	0	0
atelier	audit	0	0
atelier	cognition	0	0
atelier	flows	0	0
workshop	apps	0	0
workshop	audit	0	0
workshop	cognition	0	0
workshop	flows	0	0

COPY public.outbox_subscriber_failures (tenant, event_id, subscriber, attempts, available_at, dead_letter_at) FROM stdin;


COPY public.outbox_tenant_dispatch (tenant, served_order) FROM stdin;


COPY public.pair_evidence (tenant, left_id, right_id, order_id, event_id, simulated) FROM stdin;


COPY public.payment_attempts (id, tenant, order_id, provider, adapter_version, amount_minor, currency, state, provider_order, approval_url, capture_id, refunded_minor, revision, expires_at, created_at, environment, bn_code, provider_context, currency_scale, capture_confirmed_at) FROM stdin;


COPY public.payment_inbox (tenant, provider, event_id, data, received_at) FROM stdin;


COPY public.payment_instruments (tenant, id, customer_id, provider, adapter_version, account_ref, environment, method, reference, attempt_id, metadata, consent_version, state, created_at, revoked_at) FROM stdin;


COPY public.payment_jobs (id, tenant, attempt_id, operation, request, fingerprint, state, lease_until, attempts, available_at, response, error, created_at) FROM stdin;


COPY public.payment_provider_accounts (tenant, app, channel, account_ref, environment, ready, data, updated_at) FROM stdin;


COPY public.payment_receipt_claims (provider, environment, account_ref, kind, receipt_ref, tenant, attempt_id, job_key) FROM stdin;


COPY public.payment_refunds (tenant, job_id, attempt_id, provider_id, status, amount_minor) FROM stdin;


COPY public.platform_ai (id, revision, data, secrets, updated_by, updated_at) FROM stdin;
t	1	{}	{}	\N	<timestamp>

COPY public.platform_audit (id, actor, action, tenant, data, created_at) FROM stdin;


COPY public.platform_operators (user_id, active, granted_at) FROM stdin;


COPY public.policy (tenant, variant, views, purchases) FROM stdin;


COPY public.privacy_consent_log (id, tenant, cart_id, channel_id, data, created_at) FROM stdin;


COPY public.privacy_consents (tenant, cart_id, channel_id, policy_version, data, expires_at, source_cart_id) FROM stdin;


COPY public.private_preferences (tenant, cart_id, data, revision, updated_at) FROM stdin;


COPY public.product_assets (tenant, id, product_id, title, filename, mime, kind, public, content, digest, created_at) FROM stdin;


COPY public.product_categories (tenant, product_id, category_id) FROM stdin;


COPY public.product_channel_visibility (tenant, product_id, channel_id, visible) FROM stdin;


COPY public.product_reviews (id, tenant, product_id, session, author, rating, title, content, approved, verified, demo, created_at, source_import) FROM stdin;


COPY public.product_translations (tenant, product_id, language_id, name, description) FROM stdin;


COPY public.products (tenant, id, name, category, description, price, tax_rate, stock, revision, list_price, regulation_price, reference_price, advanced_prices, min_purchase, purchase_steps, max_purchase, parent_id, options, media, properties, delivery_days, extra, active, product_number, created_at, content_revision) FROM stdin;


COPY public.projections (event_id, tenant, kind, data, projected_at) FROM stdin;


COPY public.promotion_uses (tenant, promotion, order_id) FROM stdin;


COPY public.read_context_versions (namespace, version) FROM stdin;
languages	<uuid>

COPY public.receipt_counters (tenant, kind, next_number) FROM stdin;


COPY public.receipt_settings (tenant, data, revision) FROM stdin;


COPY public.resource_leases (id, tenant, class, slots, expires_at, created_at) FROM stdin;


COPY public.sales_channels (tenant, id, data, revision) FROM stdin;


COPY public.semantic_products (tenant, product_id, revision, embedding, embedding_model, content_hash, updated_at, vector_digest) FROM stdin;


COPY public.session_signal_events (tenant, id, session, created_at, purpose, kind, cart_id, channel_id, product_id, customer_id, dimensions, expires_at) FROM stdin;


COPY public.session_signals (tenant, session, product_id, views, cart_adds, updated_at) FROM stdin;


COPY public.shop_environments (tenant, live_tenant, name, baseline, created_at, preview_owner, preview_until) FROM stdin;


COPY public.shop_releases (id, live_tenant, environment, selections, actor, created_at) FROM stdin;


COPY public.studio_workspace_versions (tenant, user_id, revision, data, created_at) FROM stdin;


COPY public.studio_workspaces (tenant, user_id, revision, data, updated_at) FROM stdin;


COPY public.tasks (id, tenant, proposal, applied, created_at, applied_at, run_state, run_request, run_actor, run_key, run_fingerprint, run_scopes, run_credential, run_personal, run_checkpoint, run_pending, run_lease, run_until, run_revision, run_error, run_updated, conversation_id, run_trace, run_after) FROM stdin;


COPY public.tenant_ai_usage (tenant, day, attempts) FROM stdin;


COPY public.tenant_resource_limits (tenant, daily_ai, revision) FROM stdin;


COPY public.tenants (id, name, created_at, status, status_revision, status_updated_at, agents_stopped, agents_revision) FROM stdin;
atelier	Atelier	<timestamp>	active	1	<timestamp>	f	1
workshop	Workshop	<timestamp>	active	1	<timestamp>	f	1

COPY public.translation_items (tenant, job_id, product_id, revision, source, result, status, evidence) FROM stdin;


COPY public.translation_jobs (tenant, id, source_locale, target_locale, choice, overwrite, status, cursor, highwater, total, processed, lease, lease_until, error, created_at, product_ids, run_task, error_code) FROM stdin;


COPY public.user_invites (id, digest, tenant, email, role, created_by, expires_at, redeemed_at) FROM stdin;


COPY public.user_sessions (digest, user_id, default_tenant, expires_at, created_at) FROM stdin;


COPY public.vector_index_queue (tenant, kind, object_id, updated_at, revision, lease, lease_until, attempts, available_at, error_code) FROM stdin;


COPY public.workspace_roles (tenant, id, data, builtin, revision) FROM stdin;
atelier	admin	{"name": {"de-DE": "Administrator", "en-GB": "Admin", "es-ES": "Administrador", "fr-FR": "Administrateur"}, "active": true, "permissions": ["overview.read", "assistant.use", "automation.read", "automation.manage", "channels.read", "channels.manage", "developer.read", "developer.manage", "environments.read", "environments.manage", "knowledge.write", "ai.manage", "apps.read", "pos.use", "b2b.read", "b2b.manage", "catalog.read", "catalog.write", "orders.read", "orders.write", "customers.read", "customers.pii", "customers.write", "documents.read", "documents.create", "payments.read", "payments.manage", "team.manage", "settings.read", "settings.write", "apps.manage", "knowledge.read"]}	t	1
atelier	editor	{"name": {"de-DE": "Redakteur", "en-GB": "Editor", "es-ES": "Editor", "fr-FR": "Éditeur"}, "active": true, "permissions": ["catalog.read", "orders.read", "payments.read", "settings.read", "knowledge.read", "overview.read", "assistant.use", "automation.read", "channels.read", "environments.read", "apps.read", "catalog.write", "knowledge.write"]}	t	1
atelier	owner	{"name": {"de-DE": "Inhaber", "en-GB": "Owner", "es-ES": "Propietario", "fr-FR": "Propriétaire"}, "active": true, "permissions": ["overview.read", "assistant.use", "automation.read", "automation.manage", "channels.read", "channels.manage", "developer.read", "developer.manage", "environments.read", "environments.manage", "knowledge.write", "ai.manage", "apps.read", "pos.use", "b2b.read", "b2b.manage", "catalog.read", "catalog.write", "orders.read", "orders.write", "customers.read", "customers.pii", "customers.write", "documents.read", "documents.create", "payments.read", "payments.manage", "team.manage", "settings.read", "settings.write", "apps.manage", "knowledge.read"]}	t	1
atelier	viewer	{"name": {"de-DE": "Leser", "en-GB": "Viewer", "es-ES": "Lector", "fr-FR": "Lecteur"}, "active": true, "permissions": ["catalog.read", "orders.read", "payments.read", "settings.read", "knowledge.read", "overview.read", "assistant.use", "automation.read", "channels.read", "environments.read", "apps.read"]}	t	1
workshop	admin	{"name": {"de-DE": "Administrator", "en-GB": "Admin", "es-ES": "Administrador", "fr-FR": "Administrateur"}, "active": true, "permissions": ["overview.read", "assistant.use", "automation.read", "automation.manage", "channels.read", "channels.manage", "developer.read", "developer.manage", "environments.read", "environments.manage", "knowledge.write", "ai.manage", "apps.read", "pos.use", "b2b.read", "b2b.manage", "catalog.read", "catalog.write", "orders.read", "orders.write", "customers.read", "customers.pii", "customers.write", "documents.read", "documents.create", "payments.read", "payments.manage", "team.manage", "settings.read", "settings.write", "apps.manage", "knowledge.read"]}	t	1
workshop	editor	{"name": {"de-DE": "Redakteur", "en-GB": "Editor", "es-ES": "Editor", "fr-FR": "Éditeur"}, "active": true, "permissions": ["catalog.read", "orders.read", "payments.read", "settings.read", "knowledge.read", "overview.read", "assistant.use", "automation.read", "channels.read", "environments.read", "apps.read", "catalog.write", "knowledge.write"]}	t	1
workshop	owner	{"name": {"de-DE": "Inhaber", "en-GB": "Owner", "es-ES": "Propietario", "fr-FR": "Propriétaire"}, "active": true, "permissions": ["overview.read", "assistant.use", "automation.read", "automation.manage", "channels.read", "channels.manage", "developer.read", "developer.manage", "environments.read", "environments.manage", "knowledge.write", "ai.manage", "apps.read", "pos.use", "b2b.read", "b2b.manage", "catalog.read", "catalog.write", "orders.read", "orders.write", "customers.read", "customers.pii", "customers.write", "documents.read", "documents.create", "payments.read", "payments.manage", "team.manage", "settings.read", "settings.write", "apps.manage", "knowledge.read"]}	t	1
workshop	viewer	{"name": {"de-DE": "Leser", "en-GB": "Viewer", "es-ES": "Lector", "fr-FR": "Lecteur"}, "active": true, "permissions": ["catalog.read", "orders.read", "payments.read", "settings.read", "knowledge.read", "overview.read", "assistant.use", "automation.read", "channels.read", "environments.read", "apps.read"]}	t	1

COPY veyndo_authority.configuration (id, enforced, system_role, migration_role, tenant_login) FROM stdin;
t	f	\N	commerce	\N

veyndo_authority.keys: 1/1 active random 32-byte keys
