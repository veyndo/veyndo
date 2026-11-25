-- Baseline schema of the shop component (Shop configuration, domains, legal texts and the operator control plane): its tables, sequences, functions,
-- rows a fresh installation starts with, then constraints, indexes, triggers and row security.
-- Constraints and policies that reference a later component's table live in that component's file.
SET LOCAL check_function_bodies = false;

CREATE FUNCTION public.emit_read_cache_event() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
 IF TG_OP='DELETE' THEN RETURN OLD; END IF;
 INSERT INTO outbox(tenant,kind,data) VALUES(NEW.tenant,'cache.invalidate',jsonb_build_object('namespace',TG_TABLE_NAME));
 RETURN NEW;
END $$;

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

CREATE TABLE public.commerce_overrides (
    tenant text NOT NULL,
    channel_id text NOT NULL,
    data jsonb DEFAULT '[]'::jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    cache_version uuid DEFAULT gen_random_uuid() NOT NULL,
    CONSTRAINT commerce_overrides_data_check CHECK ((jsonb_typeof(data) = 'array'::text))
);

ALTER TABLE ONLY public.commerce_overrides FORCE ROW LEVEL SECURITY;

CREATE TABLE public.commerce_settings (
    tenant text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    cache_version uuid DEFAULT gen_random_uuid() NOT NULL
);

ALTER TABLE ONLY public.commerce_settings FORCE ROW LEVEL SECURITY;

CREATE TABLE public.company_logos (
    tenant text NOT NULL,
    id text NOT NULL,
    content bytea NOT NULL,
    digest text NOT NULL,
    mime text NOT NULL,
    width integer NOT NULL,
    height integer NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    -- Written with BETWEEN as in its original definition, so the stored expression tree is identical.
    CONSTRAINT company_logos_check CHECK (width BETWEEN 1 AND 4096 AND height BETWEEN 1 AND 4096),
    CONSTRAINT company_logos_content_check CHECK (((octet_length(content) >= 1) AND (octet_length(content) <= 2097152))),
    CONSTRAINT company_logos_mime_check CHECK ((mime = 'image/png'::text))
);

ALTER TABLE ONLY public.company_logos FORCE ROW LEVEL SECURITY;

CREATE TABLE public.company_overrides (
    tenant text NOT NULL,
    channel_id text NOT NULL,
    data jsonb DEFAULT '{}'::jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    CONSTRAINT company_overrides_data_check CHECK ((jsonb_typeof(data) = 'object'::text))
);

ALTER TABLE ONLY public.company_overrides FORCE ROW LEVEL SECURITY;

CREATE TABLE public.consumer_request_reviews (
    tenant text NOT NULL,
    request_id text NOT NULL,
    revision bigint NOT NULL,
    actor text NOT NULL,
    state text NOT NULL,
    note text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

ALTER TABLE ONLY public.consumer_request_reviews FORCE ROW LEVEL SECURITY;

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

ALTER TABLE ONLY public.consumer_requests FORCE ROW LEVEL SECURITY;

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

ALTER TABLE ONLY public.hosted_frontends FORCE ROW LEVEL SECURITY;

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

ALTER TABLE ONLY public.legal_acceptances FORCE ROW LEVEL SECURITY;

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

ALTER TABLE ONLY public.platform_audit FORCE ROW LEVEL SECURITY;

CREATE SEQUENCE public.platform_audit_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE public.platform_audit_id_seq OWNED BY public.platform_audit.id;

CREATE TABLE public.platform_operators (
    user_id text NOT NULL,
    active boolean DEFAULT true NOT NULL,
    granted_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.privacy_consent_log (
    id bigint NOT NULL,
    tenant text NOT NULL,
    cart_id text NOT NULL,
    channel_id text NOT NULL,
    data jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

ALTER TABLE ONLY public.privacy_consent_log FORCE ROW LEVEL SECURITY;

ALTER TABLE public.privacy_consent_log ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME public.privacy_consent_log_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
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

ALTER TABLE ONLY public.privacy_consents FORCE ROW LEVEL SECURITY;

CREATE TABLE public.tenant_resource_limits (
    tenant text NOT NULL,
    daily_ai bigint NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    CONSTRAINT tenant_resource_limits_daily_ai_check CHECK (((daily_ai >= 1) AND (daily_ai <= 1000000))),
    CONSTRAINT tenant_resource_limits_revision_check CHECK ((revision > 0))
);

ALTER TABLE ONLY public.tenant_resource_limits FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.platform_audit ALTER COLUMN id SET DEFAULT nextval('public.platform_audit_id_seq'::regclass);

-- Rows every fresh installation starts with.
INSERT INTO public.languages (id, locale, parent_id) VALUES
    ('11111111111111111111111111111111', 'de-DE', NULL),
    ('22222222222222222222222222222222', 'fr-FR', NULL),
    ('2fbb5fe2e29a4d70aa5854ce7ce3e20b', 'en-GB', NULL),
    ('33333333333333333333333333333333', 'es-ES', NULL),
    ('44444444444444444444444444444444', 'de-CH', '11111111111111111111111111111111');

INSERT INTO public.platform_ai (id, revision, data, secrets, updated_by) VALUES
    ('true', '1', '{}', '{}', NULL);

ALTER TABLE ONLY public.commerce_overrides
    ADD CONSTRAINT commerce_overrides_pkey PRIMARY KEY (tenant, channel_id);

ALTER TABLE ONLY public.commerce_settings
    ADD CONSTRAINT commerce_settings_pkey PRIMARY KEY (tenant);

ALTER TABLE ONLY public.company_logos
    ADD CONSTRAINT company_logos_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.company_logos
    ADD CONSTRAINT company_logos_tenant_digest_key UNIQUE (tenant, digest);

ALTER TABLE ONLY public.company_overrides
    ADD CONSTRAINT company_overrides_pkey PRIMARY KEY (tenant, channel_id);

ALTER TABLE ONLY public.consumer_request_reviews
    ADD CONSTRAINT consumer_request_reviews_pkey PRIMARY KEY (tenant, request_id, revision);

ALTER TABLE ONLY public.consumer_requests
    ADD CONSTRAINT consumer_requests_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.hosted_frontends
    ADD CONSTRAINT hosted_frontends_pkey PRIMARY KEY (alias);

ALTER TABLE ONLY public.languages
    ADD CONSTRAINT languages_locale_key UNIQUE (locale);

ALTER TABLE ONLY public.languages
    ADD CONSTRAINT languages_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.legal_acceptances
    ADD CONSTRAINT legal_acceptances_pkey PRIMARY KEY (tenant, cart_id);

ALTER TABLE ONLY public.platform_ai
    ADD CONSTRAINT platform_ai_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.platform_audit
    ADD CONSTRAINT platform_audit_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.platform_operators
    ADD CONSTRAINT platform_operators_pkey PRIMARY KEY (user_id);

ALTER TABLE ONLY public.privacy_consent_log
    ADD CONSTRAINT privacy_consent_log_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.privacy_consents
    ADD CONSTRAINT privacy_consents_pkey PRIMARY KEY (tenant, cart_id, channel_id);

ALTER TABLE ONLY public.tenant_resource_limits
    ADD CONSTRAINT tenant_resource_limits_pkey PRIMARY KEY (tenant);

CREATE INDEX consumer_requests_scope ON public.consumer_requests USING btree (tenant, created_at DESC, id);

CREATE INDEX hosted_frontends_app ON public.hosted_frontends USING btree (tenant, app_id) WHERE (app_id IS NOT NULL);

CREATE INDEX hosted_frontends_tenant ON public.hosted_frontends USING btree (tenant);

CREATE INDEX platform_audit_time ON public.platform_audit USING btree (created_at DESC, id DESC);

CREATE INDEX privacy_consent_log_scope ON public.privacy_consent_log USING btree (tenant, cart_id, created_at DESC);

CREATE INDEX privacy_consents_source ON public.privacy_consents USING btree (tenant, source_cart_id, channel_id) WHERE (source_cart_id IS NOT NULL);

CREATE TRIGGER commerce_overrides_cache_version BEFORE INSERT OR UPDATE ON public.commerce_overrides FOR EACH ROW EXECUTE FUNCTION public.stamp_read_context();

CREATE TRIGGER commerce_settings_cache_version BEFORE INSERT OR UPDATE ON public.commerce_settings FOR EACH ROW EXECUTE FUNCTION public.stamp_read_context();

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.commerce_overrides DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('checkoutChannel', 'channel_id');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.commerce_settings DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('settings', 'tenant');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.company_overrides DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('companyChannel', 'channel_id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.commerce_overrides FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('checkoutChannel', 'channel_id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.commerce_settings FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('settings', 'tenant');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.company_overrides FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('companyChannel', 'channel_id');

CREATE TRIGGER languages_cache_version AFTER DELETE OR UPDATE OR TRUNCATE ON public.languages FOR EACH STATEMENT EXECUTE FUNCTION public.stamp_language_registry();

CREATE TRIGGER languages_insert_cache_version AFTER INSERT ON public.languages REFERENCING NEW TABLE AS changed_languages FOR EACH STATEMENT EXECUTE FUNCTION public.stamp_new_languages();

CREATE TRIGGER overrides_cache_event AFTER INSERT OR UPDATE ON public.commerce_overrides FOR EACH ROW EXECUTE FUNCTION public.emit_read_cache_event();

CREATE TRIGGER settings_cache_event AFTER INSERT OR UPDATE ON public.commerce_settings FOR EACH ROW EXECUTE FUNCTION public.emit_read_cache_event();

ALTER TABLE ONLY public.company_logos
    ADD CONSTRAINT company_logos_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.consumer_request_reviews
    ADD CONSTRAINT consumer_request_reviews_tenant_request_id_fkey FOREIGN KEY (tenant, request_id) REFERENCES public.consumer_requests(tenant, id);

ALTER TABLE ONLY public.consumer_requests
    ADD CONSTRAINT consumer_requests_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.languages
    ADD CONSTRAINT languages_parent_id_fkey FOREIGN KEY (parent_id) REFERENCES public.languages(id);

ALTER TABLE ONLY public.legal_acceptances
    ADD CONSTRAINT legal_acceptances_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.platform_ai
    ADD CONSTRAINT platform_ai_updated_by_fkey FOREIGN KEY (updated_by) REFERENCES public.merchant_users(id);

ALTER TABLE ONLY public.platform_audit
    ADD CONSTRAINT platform_audit_actor_fkey FOREIGN KEY (actor) REFERENCES public.merchant_users(id);

ALTER TABLE ONLY public.platform_operators
    ADD CONSTRAINT platform_operators_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.merchant_users(id);

ALTER TABLE ONLY public.privacy_consent_log
    ADD CONSTRAINT privacy_consent_log_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.privacy_consents
    ADD CONSTRAINT privacy_consents_source_scope FOREIGN KEY (tenant, source_cart_id, channel_id) REFERENCES public.privacy_consents(tenant, cart_id, channel_id);

ALTER TABLE ONLY public.privacy_consents
    ADD CONSTRAINT privacy_consents_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.product_translations
    ADD CONSTRAINT product_translations_language_id_fkey FOREIGN KEY (language_id) REFERENCES public.languages(id);

ALTER TABLE ONLY public.tenant_resource_limits
    ADD CONSTRAINT tenant_resource_limits_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

CREATE POLICY authenticated_scope ON public.commerce_overrides AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.commerce_settings AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.company_logos AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.company_overrides AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.consumer_request_reviews AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.consumer_requests AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.hosted_frontends AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.legal_acceptances AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.platform_audit AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.privacy_consent_log AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.privacy_consents AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.tenant_resource_limits AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

ALTER TABLE public.commerce_overrides ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.commerce_settings ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.company_logos ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.company_overrides ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.consumer_request_reviews ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.consumer_requests ENABLE ROW LEVEL SECURITY;

CREATE POLICY core_tenant_scope ON public.commerce_overrides USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.commerce_settings USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.company_logos USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.company_overrides USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.consumer_request_reviews USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.consumer_requests USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.hosted_frontends USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.legal_acceptances USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.platform_audit USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.privacy_consent_log USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.privacy_consents USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.tenant_resource_limits USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK ((current_setting('rac.system'::text, true) = 'on'::text));

ALTER TABLE public.hosted_frontends ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.legal_acceptances ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.platform_audit ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.privacy_consent_log ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.privacy_consents ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.tenant_resource_limits ENABLE ROW LEVEL SECURITY;
