-- Baseline schema of the marketing component (Rules, coupons, promotions, flows and sales channels): its tables, sequences, functions,
-- rows a fresh installation starts with, then constraints, indexes, triggers and row security.
-- Constraints and policies that reference a later component's table live in that component's file.
SET LOCAL check_function_bodies = false;

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

ALTER TABLE ONLY public.channel_previews FORCE ROW LEVEL SECURITY;

CREATE TABLE public.commerce_flows (
    tenant text NOT NULL,
    id text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL
);

ALTER TABLE ONLY public.commerce_flows FORCE ROW LEVEL SECURITY;

CREATE TABLE public.commerce_promotions (
    tenant text NOT NULL,
    id text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL
);

ALTER TABLE ONLY public.commerce_promotions FORCE ROW LEVEL SECURITY;

CREATE TABLE public.commerce_rules (
    tenant text NOT NULL,
    id text NOT NULL,
    name jsonb NOT NULL,
    condition jsonb NOT NULL,
    active boolean DEFAULT true NOT NULL,
    revision bigint DEFAULT 1 NOT NULL
);

ALTER TABLE ONLY public.commerce_rules FORCE ROW LEVEL SECURITY;

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

ALTER TABLE ONLY public.flow_jobs FORCE ROW LEVEL SECURITY;

CREATE TABLE public.flow_steps (
    job text NOT NULL,
    node text NOT NULL,
    state text DEFAULT 'running'::text NOT NULL,
    result jsonb,
    error text,
    started_at timestamp with time zone DEFAULT now() NOT NULL,
    finished_at timestamp with time zone
);

CREATE TABLE public.promotion_uses (
    tenant text NOT NULL,
    promotion text NOT NULL,
    order_id text NOT NULL
);

ALTER TABLE ONLY public.promotion_uses FORCE ROW LEVEL SECURITY;

CREATE TABLE public.sales_channels (
    tenant text NOT NULL,
    id text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL
);

ALTER TABLE ONLY public.sales_channels FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.channel_previews
    ADD CONSTRAINT channel_previews_pkey PRIMARY KEY (digest);

ALTER TABLE ONLY public.commerce_flows
    ADD CONSTRAINT commerce_flows_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.commerce_promotions
    ADD CONSTRAINT commerce_promotions_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.commerce_rules
    ADD CONSTRAINT commerce_rules_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.flow_jobs
    ADD CONSTRAINT flow_jobs_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.flow_jobs
    ADD CONSTRAINT flow_jobs_tenant_flow_event_id_key UNIQUE (tenant, flow, event_id);

ALTER TABLE ONLY public.flow_steps
    ADD CONSTRAINT flow_steps_pkey PRIMARY KEY (job, node);

ALTER TABLE ONLY public.promotion_uses
    ADD CONSTRAINT promotion_uses_pkey PRIMARY KEY (tenant, promotion, order_id);

ALTER TABLE ONLY public.sales_channels
    ADD CONSTRAINT sales_channels_pkey PRIMARY KEY (tenant, id);

CREATE INDEX channel_previews_expiry ON public.channel_previews USING btree (expires_at);

CREATE INDEX flow_jobs_event_retention ON public.flow_jobs USING btree (event_id);

CREATE INDEX flow_jobs_expired_lease ON public.flow_jobs USING btree (lease_until) WHERE (state = 'running'::text);

CREATE INDEX flow_jobs_ready ON public.flow_jobs USING btree (available_at, event_id) WHERE (state = 'queued'::text);

CREATE INDEX flow_jobs_tenant_recent ON public.flow_jobs USING btree (tenant, event_id DESC, id);

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.commerce_flows DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('flow', 'id');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.commerce_promotions DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('promotion', 'id');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.commerce_rules DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('rule', 'id');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.sales_channels DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('channel', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.commerce_flows FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('flow', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.commerce_promotions FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('promotion', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.commerce_rules FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('rule', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.sales_channels FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('channel', 'id');

CREATE TRIGGER work_ready AFTER INSERT OR UPDATE ON public.flow_jobs FOR EACH ROW EXECUTE FUNCTION public.notify_work_ready();

ALTER TABLE ONLY public.channel_previews
    ADD CONSTRAINT channel_previews_session_digest_fkey FOREIGN KEY (session_digest) REFERENCES public.user_sessions(digest) ON DELETE CASCADE;

ALTER TABLE ONLY public.channel_previews
    ADD CONSTRAINT channel_previews_tenant_channel_fkey FOREIGN KEY (tenant, channel) REFERENCES public.sales_channels(tenant, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.commerce_overrides
    ADD CONSTRAINT commerce_overrides_tenant_channel_id_fkey FOREIGN KEY (tenant, channel_id) REFERENCES public.sales_channels(tenant, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.company_overrides
    ADD CONSTRAINT company_overrides_tenant_channel_id_fkey FOREIGN KEY (tenant, channel_id) REFERENCES public.sales_channels(tenant, id);

ALTER TABLE ONLY public.consumer_requests
    ADD CONSTRAINT consumer_requests_tenant_channel_id_fkey FOREIGN KEY (tenant, channel_id) REFERENCES public.sales_channels(tenant, id);

ALTER TABLE ONLY public.flow_jobs
    ADD CONSTRAINT flow_jobs_event_id_fkey FOREIGN KEY (event_id) REFERENCES public.outbox(id);

ALTER TABLE ONLY public.flow_jobs
    ADD CONSTRAINT flow_jobs_event_scope FOREIGN KEY (tenant, event_id) REFERENCES public.outbox(tenant, id);

ALTER TABLE ONLY public.flow_steps
    ADD CONSTRAINT flow_steps_job_fkey FOREIGN KEY (job) REFERENCES public.flow_jobs(id);

ALTER TABLE ONLY public.hosted_frontends
    ADD CONSTRAINT hosted_frontends_tenant_channel_fkey FOREIGN KEY (tenant, channel) REFERENCES public.sales_channels(tenant, id) ON DELETE RESTRICT;

ALTER TABLE ONLY public.privacy_consent_log
    ADD CONSTRAINT privacy_consent_log_tenant_channel_id_fkey FOREIGN KEY (tenant, channel_id) REFERENCES public.sales_channels(tenant, id);

ALTER TABLE ONLY public.privacy_consents
    ADD CONSTRAINT privacy_consents_tenant_channel_id_fkey FOREIGN KEY (tenant, channel_id) REFERENCES public.sales_channels(tenant, id);

ALTER TABLE ONLY public.product_channel_visibility
    ADD CONSTRAINT product_channel_visibility_tenant_channel_id_fkey FOREIGN KEY (tenant, channel_id) REFERENCES public.sales_channels(tenant, id);

ALTER TABLE ONLY public.promotion_uses
    ADD CONSTRAINT promotion_uses_tenant_promotion_fkey FOREIGN KEY (tenant, promotion) REFERENCES public.commerce_promotions(tenant, id);

CREATE POLICY authenticated_scope ON public.channel_previews AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.commerce_flows AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.commerce_promotions AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.commerce_rules AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.flow_jobs AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.promotion_uses AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.sales_channels AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY channel_preview_tenant ON public.channel_previews USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = current_setting('rac.tenant'::text, true)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = current_setting('rac.tenant'::text, true))));

ALTER TABLE public.channel_previews ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.commerce_flows ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.commerce_promotions ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.commerce_rules ENABLE ROW LEVEL SECURITY;

CREATE POLICY core_tenant_scope ON public.commerce_flows USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.commerce_promotions USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.commerce_rules USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.flow_jobs USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.promotion_uses USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.sales_channels USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

ALTER TABLE public.flow_jobs ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.promotion_uses ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.sales_channels ENABLE ROW LEVEL SECURITY;
