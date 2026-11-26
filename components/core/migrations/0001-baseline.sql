-- Baseline schema of the core component (Shared foundation: errors, request context, database scope, outbox, admission and the verified kernel): its tables, sequences, functions,
-- rows a fresh installation starts with, then constraints, indexes, triggers and row security.
-- Constraints and policies that reference a later component's table live in that component's file.
SET LOCAL check_function_bodies = false;

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

CREATE FUNCTION public.initialize_workspace_roles() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN PERFORM seed_workspace_roles(NEW.id); RETURN NEW; END $$;

CREATE FUNCTION public.notify_outbox_cache() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
 PERFORM pg_notify('veyndo_read_invalidation',json_build_object('tenant',NEW.tenant)::text);
 RETURN NEW;
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

ALTER TABLE ONLY public.channel_metrics FORCE ROW LEVEL SECURITY;

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

ALTER TABLE ONLY public.outbox FORCE ROW LEVEL SECURITY;

CREATE TABLE public.outbox_checkpoints (
    tenant text NOT NULL,
    subscriber text NOT NULL,
    last_event_id bigint DEFAULT 0 NOT NULL,
    served_order bigint DEFAULT 0 NOT NULL,
    CONSTRAINT outbox_checkpoints_last_event_id_check CHECK ((last_event_id >= 0)),
    CONSTRAINT outbox_checkpoints_served_order_check CHECK ((served_order >= 0)),
    CONSTRAINT outbox_checkpoints_subscriber_check CHECK ((subscriber = ANY (ARRAY['cognition'::text, 'apps'::text, 'flows'::text, 'audit'::text])))
);

ALTER TABLE ONLY public.outbox_checkpoints FORCE ROW LEVEL SECURITY;

CREATE TABLE public.outbox_tenant_dispatch (
    tenant text NOT NULL,
    served_order bigint DEFAULT 0 NOT NULL,
    CONSTRAINT outbox_tenant_dispatch_served_order_check CHECK ((served_order >= 0))
);

ALTER TABLE ONLY public.outbox_tenant_dispatch FORCE ROW LEVEL SECURITY;

CREATE SEQUENCE public.outbox_dispatch_sequence
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE public.outbox_dispatch_sequence OWNED BY public.outbox_tenant_dispatch.served_order;

CREATE SEQUENCE public.outbox_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE public.outbox_id_seq OWNED BY public.outbox.id;

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

ALTER TABLE ONLY public.outbox_subscriber_failures FORCE ROW LEVEL SECURITY;

CREATE TABLE public.projections (
    event_id bigint NOT NULL,
    tenant text NOT NULL,
    kind text NOT NULL,
    data jsonb NOT NULL,
    projected_at timestamp with time zone DEFAULT now() NOT NULL
);

ALTER TABLE ONLY public.projections FORCE ROW LEVEL SECURITY;

CREATE TABLE public.read_context_versions (
    namespace text NOT NULL,
    version uuid DEFAULT gen_random_uuid() NOT NULL
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

ALTER TABLE ONLY public.resource_leases FORCE ROW LEVEL SECURITY;

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

ALTER TABLE ONLY public.tenants FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.outbox ALTER COLUMN id SET DEFAULT nextval('public.outbox_id_seq'::regclass);

-- Rows every fresh installation starts with.
INSERT INTO public.outbox_checkpoints (tenant, subscriber, last_event_id, served_order) VALUES
    ('atelier', 'apps', '0', '0'),
    ('atelier', 'audit', '0', '0'),
    ('atelier', 'cognition', '0', '0'),
    ('atelier', 'flows', '0', '0'),
    ('workshop', 'apps', '0', '0'),
    ('workshop', 'audit', '0', '0'),
    ('workshop', 'cognition', '0', '0'),
    ('workshop', 'flows', '0', '0');

INSERT INTO public.read_context_versions (namespace) VALUES
    ('languages');

INSERT INTO public.tenants (id, name, status, status_revision, agents_stopped, agents_revision) VALUES
    ('atelier', 'Atelier', 'active', '1', 'false', '1'),
    ('workshop', 'Workshop', 'active', '1', 'false', '1');

ALTER TABLE ONLY public.channel_metrics
    ADD CONSTRAINT channel_metrics_pkey PRIMARY KEY (tenant, channel);

ALTER TABLE ONLY public.outbox_checkpoints
    ADD CONSTRAINT outbox_checkpoints_pkey PRIMARY KEY (tenant, subscriber);

ALTER TABLE ONLY public.outbox
    ADD CONSTRAINT outbox_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.outbox_subscriber_failures
    ADD CONSTRAINT outbox_subscriber_failures_pkey PRIMARY KEY (tenant, event_id, subscriber);

ALTER TABLE ONLY public.outbox_tenant_dispatch
    ADD CONSTRAINT outbox_tenant_dispatch_pkey PRIMARY KEY (tenant);

ALTER TABLE ONLY public.outbox
    ADD CONSTRAINT outbox_tenant_id_key UNIQUE (tenant, id);

ALTER TABLE ONLY public.projections
    ADD CONSTRAINT projections_pkey PRIMARY KEY (event_id);

ALTER TABLE ONLY public.read_context_versions
    ADD CONSTRAINT read_context_versions_pkey PRIMARY KEY (namespace);

ALTER TABLE ONLY public.resource_leases
    ADD CONSTRAINT resource_leases_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.tenants
    ADD CONSTRAINT tenants_pkey PRIMARY KEY (id);

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

CREATE INDEX resource_lease_scope ON public.resource_leases USING btree (tenant, class, expires_at);

CREATE TRIGGER outbox_cache_notification AFTER INSERT ON public.outbox FOR EACH ROW EXECUTE FUNCTION public.notify_outbox_cache();

CREATE TRIGGER prepare_outbox_delivery BEFORE INSERT ON public.outbox FOR EACH ROW EXECUTE FUNCTION public.prepare_outbox_delivery();

CREATE TRIGGER register_outbox_subscribers AFTER INSERT ON public.tenants FOR EACH ROW EXECUTE FUNCTION public.register_outbox_subscribers();

CREATE TRIGGER register_outbox_tenant AFTER INSERT ON public.outbox FOR EACH ROW EXECUTE FUNCTION public.register_outbox_tenant();

CREATE TRIGGER work_ready AFTER INSERT OR UPDATE ON public.outbox FOR EACH ROW EXECUTE FUNCTION public.notify_work_ready();

CREATE TRIGGER workspace_role_defaults AFTER INSERT ON public.tenants FOR EACH ROW EXECUTE FUNCTION public.initialize_workspace_roles();

ALTER TABLE ONLY public.outbox_checkpoints
    ADD CONSTRAINT outbox_checkpoints_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.outbox_subscriber_failures
    ADD CONSTRAINT outbox_subscriber_failures_tenant_event_id_fkey FOREIGN KEY (tenant, event_id) REFERENCES public.outbox(tenant, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.outbox_tenant_dispatch
    ADD CONSTRAINT outbox_tenant_dispatch_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.projections
    ADD CONSTRAINT projections_event_id_fkey FOREIGN KEY (event_id) REFERENCES public.outbox(id);

ALTER TABLE ONLY public.projections
    ADD CONSTRAINT projections_event_scope FOREIGN KEY (tenant, event_id) REFERENCES public.outbox(tenant, id);

CREATE POLICY authenticated_scope ON public.channel_metrics AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.outbox AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.outbox_checkpoints AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.outbox_subscriber_failures AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.outbox_tenant_dispatch AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.projections AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.resource_leases AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_tenant ON public.tenants USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (id = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK (COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false));

ALTER TABLE public.channel_metrics ENABLE ROW LEVEL SECURITY;

CREATE POLICY core_tenant_scope ON public.channel_metrics USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.outbox USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.outbox_checkpoints USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.outbox_subscriber_failures USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.outbox_tenant_dispatch USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.projections USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.resource_leases USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

ALTER TABLE public.outbox ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.outbox_checkpoints ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.outbox_subscriber_failures ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.outbox_tenant_dispatch ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.projections ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.resource_leases ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.tenants ENABLE ROW LEVEL SECURITY;

-- A runtime role that already exists before setup gets its grants here; deploy/sql/provision-runtime.sh
-- grants every table after setup in the usual order.
DO $fn$ BEGIN
 IF EXISTS(SELECT 1 FROM pg_roles WHERE rolname='veyndo_runtime') THEN
  GRANT SELECT,INSERT,UPDATE,DELETE ON outbox_tenant_dispatch TO veyndo_runtime;
  GRANT USAGE,SELECT ON SEQUENCE outbox_dispatch_sequence TO veyndo_runtime;
 END IF;
END $fn$;

-- shop_environments (owner core)

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

ALTER TABLE ONLY public.shop_environments FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.shop_environments
    ADD CONSTRAINT environments_live_tenant_key UNIQUE (live_tenant, tenant);

ALTER TABLE ONLY public.shop_environments
    ADD CONSTRAINT shop_environments_pkey PRIMARY KEY (tenant);

CREATE UNIQUE INDEX app_preview_owner ON public.shop_environments USING btree (live_tenant, preview_owner) WHERE (preview_owner IS NOT NULL);

CREATE INDEX environments_live ON public.shop_environments USING btree (live_tenant);

ALTER TABLE ONLY public.shop_environments
    ADD CONSTRAINT shop_environments_live_tenant_fkey FOREIGN KEY (live_tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.shop_environments
    ADD CONSTRAINT shop_environments_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

CREATE POLICY authenticated_scope ON public.shop_environments AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY core_tenant_scope ON public.shop_environments USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

ALTER TABLE public.shop_environments ENABLE ROW LEVEL SECURITY;

-- tenant_ai_usage (owner core)

CREATE TABLE public.tenant_ai_usage (
    tenant text NOT NULL,
    day date NOT NULL,
    attempts bigint NOT NULL,
    CONSTRAINT tenant_ai_usage_attempts_check CHECK ((attempts > 0))
);

ALTER TABLE ONLY public.tenant_ai_usage FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.tenant_ai_usage
    ADD CONSTRAINT tenant_ai_usage_pkey PRIMARY KEY (tenant, day);

ALTER TABLE ONLY public.tenant_ai_usage
    ADD CONSTRAINT tenant_ai_usage_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

CREATE POLICY authenticated_scope ON public.tenant_ai_usage AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY core_tenant_scope ON public.tenant_ai_usage USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

ALTER TABLE public.tenant_ai_usage ENABLE ROW LEVEL SECURITY;

-- tenant_resource_limits (owner core)

CREATE TABLE public.tenant_resource_limits (
    tenant text NOT NULL,
    daily_ai bigint NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    CONSTRAINT tenant_resource_limits_daily_ai_check CHECK (((daily_ai >= 1) AND (daily_ai <= 1000000))),
    CONSTRAINT tenant_resource_limits_revision_check CHECK ((revision > 0))
);

ALTER TABLE ONLY public.tenant_resource_limits FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.tenant_resource_limits
    ADD CONSTRAINT tenant_resource_limits_pkey PRIMARY KEY (tenant);

ALTER TABLE ONLY public.tenant_resource_limits
    ADD CONSTRAINT tenant_resource_limits_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

CREATE POLICY authenticated_scope ON public.tenant_resource_limits AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY core_tenant_scope ON public.tenant_resource_limits USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK ((current_setting('rac.system'::text, true) = 'on'::text));

ALTER TABLE public.tenant_resource_limits ENABLE ROW LEVEL SECURITY;
