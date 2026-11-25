-- Baseline schema of the payments component (Provider-independent payment ledger, captures, refunds and provider workers): its tables, sequences, functions,
-- rows a fresh installation starts with, then constraints, indexes, triggers and row security.
-- Constraints and policies that reference a later component's table live in that component's file.
SET LOCAL check_function_bodies = false;

CREATE FUNCTION public.intelligence_capture_clock() RETURNS trigger
    LANGUAGE plpgsql
    AS $$BEGIN
 IF NEW.environment='live' AND NEW.state IN ('captured','captured_late') AND NEW.capture_id IS NOT NULL
 AND NEW.capture_confirmed_at IS NULL THEN NEW.capture_confirmed_at=now();END IF;
 IF TG_OP='UPDATE' AND OLD.capture_confirmed_at IS NOT NULL THEN NEW.capture_confirmed_at=OLD.capture_confirmed_at;END IF;
 RETURN NEW;END$$;

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

ALTER TABLE ONLY public.payment_attempts FORCE ROW LEVEL SECURITY;

CREATE TABLE public.payment_inbox (
    tenant text NOT NULL,
    provider text NOT NULL,
    event_id text NOT NULL,
    data jsonb NOT NULL,
    received_at timestamp with time zone DEFAULT now() NOT NULL
);

ALTER TABLE ONLY public.payment_inbox FORCE ROW LEVEL SECURITY;

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
    -- Written with BETWEEN as in its original definition, so the stored expression tree is identical.
    CONSTRAINT payment_instruments_check CHECK (length(reference) BETWEEN 1 AND 200 AND length(consent_version) BETWEEN 1 AND 50),
    CONSTRAINT payment_instruments_environment_check CHECK ((environment = ANY (ARRAY['sandbox'::text, 'live'::text, 'simulation'::text, 'contract-fixture'::text]))),
    CONSTRAINT payment_instruments_state_check CHECK ((state = ANY (ARRAY['active'::text, 'revoked'::text])))
);

ALTER TABLE ONLY public.payment_instruments FORCE ROW LEVEL SECURITY;

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

ALTER TABLE ONLY public.payment_jobs FORCE ROW LEVEL SECURITY;

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

ALTER TABLE ONLY public.payment_provider_accounts FORCE ROW LEVEL SECURITY;

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

ALTER TABLE ONLY public.payment_receipt_claims FORCE ROW LEVEL SECURITY;

CREATE TABLE public.payment_refunds (
    tenant text NOT NULL,
    job_id text NOT NULL,
    attempt_id text NOT NULL,
    provider_id text NOT NULL,
    status text NOT NULL,
    amount_minor bigint NOT NULL
);

ALTER TABLE ONLY public.payment_refunds FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.payment_attempts
    ADD CONSTRAINT payment_attempts_order_id_key UNIQUE (order_id);

ALTER TABLE ONLY public.payment_attempts
    ADD CONSTRAINT payment_attempts_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.payment_attempts
    ADD CONSTRAINT payment_attempts_tenant_id_key UNIQUE (tenant, id);

ALTER TABLE ONLY public.payment_inbox
    ADD CONSTRAINT payment_inbox_pkey PRIMARY KEY (tenant, provider, event_id);

ALTER TABLE ONLY public.payment_instruments
    ADD CONSTRAINT payment_instruments_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.payment_instruments
    ADD CONSTRAINT payment_instruments_tenant_provider_environment_account_ref_key UNIQUE (tenant, provider, environment, account_ref, reference);

ALTER TABLE ONLY public.payment_jobs
    ADD CONSTRAINT payment_jobs_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.payment_jobs
    ADD CONSTRAINT payment_jobs_tenant_id_attempt_key UNIQUE (tenant, id, attempt_id);

ALTER TABLE ONLY public.payment_jobs
    ADD CONSTRAINT payment_jobs_tenant_id_key UNIQUE (tenant, id);

ALTER TABLE ONLY public.payment_provider_accounts
    ADD CONSTRAINT payment_provider_accounts_pkey PRIMARY KEY (tenant, app, channel);

ALTER TABLE ONLY public.payment_receipt_claims
    ADD CONSTRAINT payment_receipt_claims_pkey PRIMARY KEY (provider, environment, account_ref, kind, receipt_ref);

ALTER TABLE ONLY public.payment_refunds
    ADD CONSTRAINT payment_refunds_pkey PRIMARY KEY (tenant, job_id);

ALTER TABLE ONLY public.payment_refunds
    ADD CONSTRAINT payment_refunds_tenant_provider_id_key UNIQUE (tenant, provider_id);

CREATE INDEX intelligence_live_payment_cart ON public.payment_attempts USING btree (tenant, order_id) WHERE ((environment = 'live'::text) AND (state = ANY (ARRAY['captured'::text, 'captured_late'::text, 'refunded'::text])));

CREATE INDEX payment_instruments_customer ON public.payment_instruments USING btree (tenant, customer_id, state, created_at);

CREATE INDEX payment_jobs_pending ON public.payment_jobs USING btree (available_at) WHERE (state = ANY (ARRAY['queued'::text, 'running'::text]));

CREATE TRIGGER intelligence_capture_clock BEFORE INSERT OR UPDATE ON public.payment_attempts FOR EACH ROW EXECUTE FUNCTION public.intelligence_capture_clock();

CREATE TRIGGER work_ready AFTER INSERT OR UPDATE ON public.payment_jobs FOR EACH ROW EXECUTE FUNCTION public.notify_work_ready();

ALTER TABLE ONLY public.inventory_reservations
    ADD CONSTRAINT inventory_reservations_attempt_id_fkey FOREIGN KEY (attempt_id) REFERENCES public.payment_attempts(id);

ALTER TABLE ONLY public.payment_attempts
    ADD CONSTRAINT payment_attempts_order_id_fkey FOREIGN KEY (order_id) REFERENCES public.orders(id);

ALTER TABLE ONLY public.payment_attempts
    ADD CONSTRAINT payment_attempts_order_scope FOREIGN KEY (tenant, order_id) REFERENCES public.orders(tenant, id);

ALTER TABLE ONLY public.payment_instruments
    ADD CONSTRAINT payment_instruments_tenant_attempt_id_fkey FOREIGN KEY (tenant, attempt_id) REFERENCES public.payment_attempts(tenant, id);

ALTER TABLE ONLY public.payment_instruments
    ADD CONSTRAINT payment_instruments_tenant_customer_id_fkey FOREIGN KEY (tenant, customer_id) REFERENCES public.customers(tenant, id);

ALTER TABLE ONLY public.payment_jobs
    ADD CONSTRAINT payment_jobs_attempt_id_fkey FOREIGN KEY (attempt_id) REFERENCES public.payment_attempts(id);

ALTER TABLE ONLY public.payment_jobs
    ADD CONSTRAINT payment_jobs_attempt_scope FOREIGN KEY (tenant, attempt_id) REFERENCES public.payment_attempts(tenant, id);

ALTER TABLE ONLY public.payment_provider_accounts
    ADD CONSTRAINT payment_provider_accounts_tenant_channel_fkey FOREIGN KEY (tenant, channel) REFERENCES public.sales_channels(tenant, id);

ALTER TABLE ONLY public.payment_receipt_claims
    ADD CONSTRAINT payment_receipt_claims_tenant_attempt_id_fkey FOREIGN KEY (tenant, attempt_id) REFERENCES public.payment_attempts(tenant, id);

ALTER TABLE ONLY public.payment_refunds
    ADD CONSTRAINT refunds_job_attempt_scope FOREIGN KEY (tenant, job_id, attempt_id) REFERENCES public.payment_jobs(tenant, id, attempt_id);

ALTER TABLE ONLY public.inventory_reservations
    ADD CONSTRAINT reservations_attempt_scope FOREIGN KEY (tenant, attempt_id) REFERENCES public.payment_attempts(tenant, id);

CREATE POLICY authenticated_scope ON public.payment_attempts AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.payment_inbox AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.payment_instruments AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.payment_jobs AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.payment_provider_accounts AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.payment_receipt_claims AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.payment_refunds AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY core_tenant_scope ON public.payment_attempts USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.payment_inbox USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.payment_instruments USING (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text))) WITH CHECK (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text)));

CREATE POLICY core_tenant_scope ON public.payment_jobs USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.payment_provider_accounts USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.payment_receipt_claims USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.payment_refunds USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

ALTER TABLE public.payment_attempts ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.payment_inbox ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.payment_instruments ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.payment_jobs ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.payment_provider_accounts ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.payment_receipt_claims ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.payment_refunds ENABLE ROW LEVEL SECURITY;
