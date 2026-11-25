-- Baseline schema of the pricing component (Money, price calculation, discounts, currencies, taxes and customer groups): its tables, sequences, functions,
-- rows a fresh installation starts with, then constraints, indexes, triggers and row security.
-- Constraints and policies that reference a later component's table live in that component's file.
SET LOCAL check_function_bodies = false;

CREATE TABLE public.commerce_customer_groups (
    tenant text NOT NULL,
    id text NOT NULL,
    data jsonb DEFAULT '{}'::jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL
);

ALTER TABLE ONLY public.commerce_customer_groups FORCE ROW LEVEL SECURITY;

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

ALTER TABLE ONLY public.currency_price_jobs FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.commerce_customer_groups
    ADD CONSTRAINT commerce_customer_groups_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.currency_price_jobs
    ADD CONSTRAINT currency_price_jobs_pkey PRIMARY KEY (tenant, id);

CREATE INDEX currency_price_work ON public.currency_price_jobs USING btree (created_at) WHERE (state = 'queued'::text);

CREATE TRIGGER work_ready AFTER INSERT OR UPDATE ON public.currency_price_jobs FOR EACH ROW EXECUTE FUNCTION public.notify_work_ready();

ALTER TABLE ONLY public.currency_price_jobs
    ADD CONSTRAINT currency_price_jobs_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id) ON DELETE CASCADE;

CREATE POLICY authenticated_scope ON public.commerce_customer_groups AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.currency_price_jobs AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

ALTER TABLE public.commerce_customer_groups ENABLE ROW LEVEL SECURITY;

CREATE POLICY core_tenant_scope ON public.commerce_customer_groups USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

ALTER TABLE public.currency_price_jobs ENABLE ROW LEVEL SECURITY;

CREATE POLICY currency_price_scope ON public.currency_price_jobs USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));
