-- Baseline schema of the identity component (Merchant accounts, memberships, sessions, roles and route policy): its tables, sequences, functions,
-- rows a fresh installation starts with, then constraints, indexes, triggers and row security.
-- Constraints and policies that reference a later component's table live in that component's file.
SET LOCAL check_function_bodies = false;

CREATE FUNCTION public.seed_workspace_roles(t text) RETURNS void
    LANGUAGE sql
    AS $$
 INSERT INTO workspace_roles(tenant,id,data,builtin) VALUES
(t,'owner',jsonb_build_object('name',jsonb_build_object('en-GB','Owner','de-DE','Inhaber','es-ES','Propietario','fr-FR','Propriétaire'),'permissions','["overview.read", "assistant.use", "automation.read", "automation.manage", "channels.read", "channels.manage", "developer.read", "developer.manage", "environments.read", "environments.manage", "knowledge.write", "ai.manage", "apps.read", "pos.use", "b2b.read", "b2b.manage", "catalog.read", "catalog.write", "orders.read", "orders.write", "customers.read", "customers.pii", "customers.write", "documents.read", "documents.create", "payments.read", "payments.manage", "team.manage", "settings.read", "settings.write", "apps.manage", "knowledge.read"]'::jsonb,'active',true),true),
(t,'admin',jsonb_build_object('name',jsonb_build_object('en-GB','Admin','de-DE','Administrator','es-ES','Administrador','fr-FR','Administrateur'),'permissions','["overview.read", "assistant.use", "automation.read", "automation.manage", "channels.read", "channels.manage", "developer.read", "developer.manage", "environments.read", "environments.manage", "knowledge.write", "ai.manage", "apps.read", "pos.use", "b2b.read", "b2b.manage", "catalog.read", "catalog.write", "orders.read", "orders.write", "customers.read", "customers.pii", "customers.write", "documents.read", "documents.create", "payments.read", "payments.manage", "team.manage", "settings.read", "settings.write", "apps.manage", "knowledge.read"]'::jsonb,'active',true),true),
(t,'editor',jsonb_build_object('name',jsonb_build_object('en-GB','Editor','de-DE','Redakteur','es-ES','Editor','fr-FR','Éditeur'),'permissions','["catalog.read", "orders.read", "payments.read", "settings.read", "knowledge.read", "overview.read", "assistant.use", "automation.read", "channels.read", "environments.read", "apps.read", "catalog.write", "knowledge.write"]'::jsonb,'active',true),true),
(t,'viewer',jsonb_build_object('name',jsonb_build_object('en-GB','Viewer','de-DE','Leser','es-ES','Lector','fr-FR','Lecteur'),'permissions','["catalog.read", "orders.read", "payments.read", "settings.read", "knowledge.read", "overview.read", "assistant.use", "automation.read", "channels.read", "environments.read", "apps.read"]'::jsonb,'active',true),true) ON CONFLICT DO NOTHING;
$$;

CREATE TABLE public.auth_attempt_buckets (
    key text NOT NULL,
    window_at timestamp with time zone DEFAULT now() NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    blocked_until timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
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

ALTER TABLE ONLY public.integration_keys FORCE ROW LEVEL SECURITY;

CREATE TABLE public.memberships (
    user_id text NOT NULL,
    tenant text NOT NULL,
    role text NOT NULL,
    active boolean DEFAULT true NOT NULL,
    permissions jsonb DEFAULT 'null'::jsonb NOT NULL
);

ALTER TABLE ONLY public.memberships FORCE ROW LEVEL SECURITY;

CREATE TABLE public.workspace_roles (
    tenant text NOT NULL,
    id text NOT NULL,
    data jsonb NOT NULL,
    builtin boolean DEFAULT false NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    CONSTRAINT workspace_roles_id_check CHECK ((id ~ '^[a-z][a-z0-9_]{0,31}$'::text)),
    CONSTRAINT workspace_roles_revision_check CHECK ((revision > 0))
);

ALTER TABLE ONLY public.workspace_roles FORCE ROW LEVEL SECURITY;

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

CREATE TABLE public.merchant_handoffs (
    digest text NOT NULL,
    user_id text NOT NULL,
    tenant text NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    session_digest text
);

ALTER TABLE ONLY public.merchant_handoffs FORCE ROW LEVEL SECURITY;

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

ALTER TABLE ONLY public.merchant_mfa FORCE ROW LEVEL SECURITY;

CREATE TABLE public.merchant_mfa_challenges (
    digest text NOT NULL,
    user_id text NOT NULL,
    tenant text NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    expires_at timestamp with time zone DEFAULT (now() + '00:05:00'::interval) NOT NULL
);

ALTER TABLE ONLY public.merchant_mfa_challenges FORCE ROW LEVEL SECURITY;

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

ALTER TABLE ONLY public.merchant_users FORCE ROW LEVEL SECURITY;

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

ALTER TABLE ONLY public.user_invites FORCE ROW LEVEL SECURITY;

CREATE TABLE public.user_sessions (
    digest text NOT NULL,
    user_id text NOT NULL,
    default_tenant text NOT NULL,
    expires_at timestamp with time zone DEFAULT (now() + '12:00:00'::interval) NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

-- Rows every fresh installation starts with.
INSERT INTO public.workspace_roles (tenant, id, data, builtin, revision) VALUES
    ('atelier', 'admin', '{"name": {"de-DE": "Administrator", "en-GB": "Admin", "es-ES": "Administrador", "fr-FR": "Administrateur"}, "active": true, "permissions": ["overview.read", "assistant.use", "automation.read", "automation.manage", "channels.read", "channels.manage", "developer.read", "developer.manage", "environments.read", "environments.manage", "knowledge.write", "ai.manage", "apps.read", "pos.use", "b2b.read", "b2b.manage", "catalog.read", "catalog.write", "orders.read", "orders.write", "customers.read", "customers.pii", "customers.write", "documents.read", "documents.create", "payments.read", "payments.manage", "team.manage", "settings.read", "settings.write", "apps.manage", "knowledge.read"]}', 'true', '1'),
    ('atelier', 'editor', '{"name": {"de-DE": "Redakteur", "en-GB": "Editor", "es-ES": "Editor", "fr-FR": "Éditeur"}, "active": true, "permissions": ["catalog.read", "orders.read", "payments.read", "settings.read", "knowledge.read", "overview.read", "assistant.use", "automation.read", "channels.read", "environments.read", "apps.read", "catalog.write", "knowledge.write"]}', 'true', '1'),
    ('atelier', 'owner', '{"name": {"de-DE": "Inhaber", "en-GB": "Owner", "es-ES": "Propietario", "fr-FR": "Propriétaire"}, "active": true, "permissions": ["overview.read", "assistant.use", "automation.read", "automation.manage", "channels.read", "channels.manage", "developer.read", "developer.manage", "environments.read", "environments.manage", "knowledge.write", "ai.manage", "apps.read", "pos.use", "b2b.read", "b2b.manage", "catalog.read", "catalog.write", "orders.read", "orders.write", "customers.read", "customers.pii", "customers.write", "documents.read", "documents.create", "payments.read", "payments.manage", "team.manage", "settings.read", "settings.write", "apps.manage", "knowledge.read"]}', 'true', '1'),
    ('atelier', 'viewer', '{"name": {"de-DE": "Leser", "en-GB": "Viewer", "es-ES": "Lector", "fr-FR": "Lecteur"}, "active": true, "permissions": ["catalog.read", "orders.read", "payments.read", "settings.read", "knowledge.read", "overview.read", "assistant.use", "automation.read", "channels.read", "environments.read", "apps.read"]}', 'true', '1'),
    ('workshop', 'admin', '{"name": {"de-DE": "Administrator", "en-GB": "Admin", "es-ES": "Administrador", "fr-FR": "Administrateur"}, "active": true, "permissions": ["overview.read", "assistant.use", "automation.read", "automation.manage", "channels.read", "channels.manage", "developer.read", "developer.manage", "environments.read", "environments.manage", "knowledge.write", "ai.manage", "apps.read", "pos.use", "b2b.read", "b2b.manage", "catalog.read", "catalog.write", "orders.read", "orders.write", "customers.read", "customers.pii", "customers.write", "documents.read", "documents.create", "payments.read", "payments.manage", "team.manage", "settings.read", "settings.write", "apps.manage", "knowledge.read"]}', 'true', '1'),
    ('workshop', 'editor', '{"name": {"de-DE": "Redakteur", "en-GB": "Editor", "es-ES": "Editor", "fr-FR": "Éditeur"}, "active": true, "permissions": ["catalog.read", "orders.read", "payments.read", "settings.read", "knowledge.read", "overview.read", "assistant.use", "automation.read", "channels.read", "environments.read", "apps.read", "catalog.write", "knowledge.write"]}', 'true', '1'),
    ('workshop', 'owner', '{"name": {"de-DE": "Inhaber", "en-GB": "Owner", "es-ES": "Propietario", "fr-FR": "Propriétaire"}, "active": true, "permissions": ["overview.read", "assistant.use", "automation.read", "automation.manage", "channels.read", "channels.manage", "developer.read", "developer.manage", "environments.read", "environments.manage", "knowledge.write", "ai.manage", "apps.read", "pos.use", "b2b.read", "b2b.manage", "catalog.read", "catalog.write", "orders.read", "orders.write", "customers.read", "customers.pii", "customers.write", "documents.read", "documents.create", "payments.read", "payments.manage", "team.manage", "settings.read", "settings.write", "apps.manage", "knowledge.read"]}', 'true', '1'),
    ('workshop', 'viewer', '{"name": {"de-DE": "Leser", "en-GB": "Viewer", "es-ES": "Lector", "fr-FR": "Lecteur"}, "active": true, "permissions": ["catalog.read", "orders.read", "payments.read", "settings.read", "knowledge.read", "overview.read", "assistant.use", "automation.read", "channels.read", "environments.read", "apps.read"]}', 'true', '1');

ALTER TABLE ONLY public.auth_attempt_buckets
    ADD CONSTRAINT auth_attempt_buckets_pkey PRIMARY KEY (key);

ALTER TABLE ONLY public.identity_inference_daily
    ADD CONSTRAINT identity_inference_daily_pkey PRIMARY KEY (email, day);

ALTER TABLE ONLY public.identity_nonces
    ADD CONSTRAINT identity_nonces_pkey PRIMARY KEY (digest);

ALTER TABLE ONLY public.integration_keys
    ADD CONSTRAINT integration_keys_digest_key UNIQUE (digest);

ALTER TABLE ONLY public.integration_keys
    ADD CONSTRAINT integration_keys_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.memberships
    ADD CONSTRAINT memberships_pkey PRIMARY KEY (user_id, tenant);

ALTER TABLE ONLY public.merchant_handoffs
    ADD CONSTRAINT merchant_handoffs_pkey PRIMARY KEY (digest);

ALTER TABLE ONLY public.merchant_identities
    ADD CONSTRAINT merchant_identities_pkey PRIMARY KEY (issuer, subject);

ALTER TABLE ONLY public.merchant_mfa_challenges
    ADD CONSTRAINT merchant_mfa_challenges_pkey PRIMARY KEY (digest);

ALTER TABLE ONLY public.merchant_mfa
    ADD CONSTRAINT merchant_mfa_pkey PRIMARY KEY (user_id);

ALTER TABLE ONLY public.merchant_users
    ADD CONSTRAINT merchant_users_email_key UNIQUE (email);

ALTER TABLE ONLY public.merchant_users
    ADD CONSTRAINT merchant_users_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.user_invites
    ADD CONSTRAINT user_invites_digest_key UNIQUE (digest);

ALTER TABLE ONLY public.user_invites
    ADD CONSTRAINT user_invites_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.user_sessions
    ADD CONSTRAINT user_sessions_pkey PRIMARY KEY (digest);

ALTER TABLE ONLY public.workspace_roles
    ADD CONSTRAINT workspace_roles_pkey PRIMARY KEY (tenant, id);

CREATE INDEX integration_app_active ON public.integration_keys USING btree (tenant, app_id, expires_at) WHERE (app_id IS NOT NULL);

CREATE INDEX integration_keys_identity ON public.integration_keys USING btree (user_id, tenant);

CREATE INDEX live_sessions ON public.user_sessions USING btree (user_id, expires_at);

CREATE INDEX membership_tenant ON public.memberships USING btree (tenant, user_id) WHERE active;

CREATE INDEX merchant_mfa_challenge_expiry ON public.merchant_mfa_challenges USING btree (expires_at);

CREATE TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.workspace_roles FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('role', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.workspace_roles FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('role', 'id');

ALTER TABLE ONLY public.integration_keys
    ADD CONSTRAINT integration_keys_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.integration_keys
    ADD CONSTRAINT integration_keys_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.merchant_users(id);

ALTER TABLE ONLY public.memberships
    ADD CONSTRAINT memberships_role_tenant_fk FOREIGN KEY (tenant, role) REFERENCES public.workspace_roles(tenant, id);

ALTER TABLE ONLY public.memberships
    ADD CONSTRAINT memberships_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.memberships
    ADD CONSTRAINT memberships_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.merchant_users(id);

ALTER TABLE ONLY public.merchant_handoffs
    ADD CONSTRAINT merchant_handoffs_session_digest_fkey FOREIGN KEY (session_digest) REFERENCES public.user_sessions(digest) ON DELETE CASCADE;

ALTER TABLE ONLY public.merchant_handoffs
    ADD CONSTRAINT merchant_handoffs_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.merchant_handoffs
    ADD CONSTRAINT merchant_handoffs_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.merchant_users(id);

ALTER TABLE ONLY public.merchant_identities
    ADD CONSTRAINT merchant_identities_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.merchant_users(id);

ALTER TABLE ONLY public.merchant_mfa_challenges
    ADD CONSTRAINT merchant_mfa_challenges_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.merchant_mfa_challenges
    ADD CONSTRAINT merchant_mfa_challenges_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.merchant_users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.merchant_mfa
    ADD CONSTRAINT merchant_mfa_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.merchant_users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.user_invites
    ADD CONSTRAINT user_invites_role_tenant_fk FOREIGN KEY (tenant, role) REFERENCES public.workspace_roles(tenant, id);

ALTER TABLE ONLY public.user_invites
    ADD CONSTRAINT user_invites_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.user_sessions
    ADD CONSTRAINT user_sessions_default_tenant_fkey FOREIGN KEY (default_tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.user_sessions
    ADD CONSTRAINT user_sessions_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.merchant_users(id);

ALTER TABLE ONLY public.workspace_roles
    ADD CONSTRAINT workspace_roles_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

CREATE POLICY authenticated_member ON public.merchant_users USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (EXISTS ( SELECT 1
   FROM public.memberships m
  WHERE ((m.user_id = merchant_users.id) AND (m.tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))))))) WITH CHECK (COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false));

CREATE POLICY authenticated_scope ON public.integration_keys AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.memberships AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.merchant_handoffs AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.merchant_mfa_challenges AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.user_invites AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.workspace_roles AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY core_tenant_scope ON public.integration_keys USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.memberships USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.merchant_handoffs USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.merchant_mfa_challenges USING ((current_setting('rac.system'::text, true) = 'on'::text)) WITH CHECK ((current_setting('rac.system'::text, true) = 'on'::text));

CREATE POLICY core_tenant_scope ON public.user_invites USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.workspace_roles USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY identity_system ON public.merchant_mfa USING ((current_setting('rac.system'::text, true) = 'on'::text)) WITH CHECK ((current_setting('rac.system'::text, true) = 'on'::text));

ALTER TABLE public.integration_keys ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.memberships ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.merchant_handoffs ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.merchant_mfa ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.merchant_mfa_challenges ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.merchant_users ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.user_invites ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.workspace_roles ENABLE ROW LEVEL SECURITY;

-- A runtime role that already exists before setup gets its grants here; deploy/sql/provision-runtime.sh
-- grants every table after setup in the usual order.
DO $fn$ BEGIN
 IF EXISTS(SELECT 1 FROM pg_roles WHERE rolname='veyndo_runtime') THEN
  GRANT SELECT,INSERT,UPDATE,DELETE,REFERENCES ON workspace_roles TO veyndo_runtime;
  GRANT SELECT ON membership_grants TO veyndo_runtime;
 END IF;
END $fn$;
