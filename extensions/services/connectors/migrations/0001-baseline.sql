-- Baseline schema of the connectors component (Standard provider services and their delivery queue; hosted by the separate connector binary in extensions/services/connectors): its tables, sequences, functions,
-- rows a fresh installation starts with, then constraints, indexes, triggers and row security.
-- Constraints and policies that reference a later component's table live in that component's file.
SET LOCAL check_function_bodies = false;

CREATE TABLE public.connector_changes (
    seq bigint NOT NULL,
    tenant text NOT NULL,
    app text NOT NULL,
    data bytea NOT NULL
);

ALTER TABLE ONLY public.connector_changes FORCE ROW LEVEL SECURITY;

ALTER TABLE public.connector_changes ALTER COLUMN seq ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME public.connector_changes_seq_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);

CREATE TABLE public.connector_config (
    tenant text NOT NULL,
    app text NOT NULL,
    data bytea NOT NULL
);

ALTER TABLE ONLY public.connector_config FORCE ROW LEVEL SECURITY;

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

ALTER TABLE ONLY public.connector_jobs FORCE ROW LEVEL SECURITY;

CREATE TABLE public.connector_limits (
    tenant text NOT NULL,
    app text NOT NULL,
    day date DEFAULT CURRENT_DATE NOT NULL,
    enqueued bigint DEFAULT 0 NOT NULL,
    minute timestamp with time zone DEFAULT date_trunc('minute'::text, now()) NOT NULL,
    dispatched bigint DEFAULT 0 NOT NULL,
    last_started timestamp with time zone DEFAULT '1970-01-01 00:00:00+00'::timestamp with time zone NOT NULL
);

ALTER TABLE ONLY public.connector_limits FORCE ROW LEVEL SECURITY;

CREATE TABLE public.connector_oauth (
    digest text NOT NULL,
    tenant text NOT NULL,
    app text NOT NULL,
    data bytea NOT NULL,
    expires_at timestamp with time zone NOT NULL
);

ALTER TABLE ONLY public.connector_oauth FORCE ROW LEVEL SECURITY;

CREATE TABLE public.connector_sources (
    tenant text NOT NULL,
    app text NOT NULL,
    id text NOT NULL,
    data bytea NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

ALTER TABLE ONLY public.connector_sources FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.connector_changes
    ADD CONSTRAINT connector_changes_pkey PRIMARY KEY (seq);

ALTER TABLE ONLY public.connector_config
    ADD CONSTRAINT connector_config_pkey PRIMARY KEY (tenant, app);

ALTER TABLE ONLY public.connector_jobs
    ADD CONSTRAINT connector_jobs_pkey PRIMARY KEY (tenant, app, id);

ALTER TABLE ONLY public.connector_limits
    ADD CONSTRAINT connector_limits_pkey PRIMARY KEY (tenant, app);

ALTER TABLE ONLY public.connector_oauth
    ADD CONSTRAINT connector_oauth_pkey PRIMARY KEY (digest);

ALTER TABLE ONLY public.connector_sources
    ADD CONSTRAINT connector_sources_pkey PRIMARY KEY (tenant, app, id);

CREATE INDEX connector_changes_scope ON public.connector_changes USING btree (tenant, app, seq);

CREATE INDEX connector_job_claim ON public.connector_jobs USING btree (app, available, tenant) WHERE (state = 'queued'::text);

CREATE INDEX connector_job_leases ON public.connector_jobs USING btree (lease_until) WHERE (state = 'running'::text);

ALTER TABLE ONLY public.connector_changes
    ADD CONSTRAINT connector_changes_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.connector_config
    ADD CONSTRAINT connector_config_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.connector_jobs
    ADD CONSTRAINT connector_jobs_tenant_app_fkey FOREIGN KEY (tenant, app) REFERENCES public.connector_limits(tenant, app) ON DELETE CASCADE;

ALTER TABLE ONLY public.connector_limits
    ADD CONSTRAINT connector_limits_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.connector_oauth
    ADD CONSTRAINT connector_oauth_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.connector_sources
    ADD CONSTRAINT connector_sources_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id) ON DELETE CASCADE;

ALTER TABLE public.connector_changes ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.connector_config ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.connector_jobs ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.connector_limits ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.connector_oauth ENABLE ROW LEVEL SECURITY;

CREATE POLICY connector_scope ON public.connector_changes USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY connector_scope ON public.connector_config USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY connector_scope ON public.connector_jobs USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY connector_scope ON public.connector_limits USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY connector_scope ON public.connector_oauth USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY connector_scope ON public.connector_sources USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

ALTER TABLE public.connector_sources ENABLE ROW LEVEL SECURITY;
