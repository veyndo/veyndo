-- Baseline schema of the studio component (Merchant overview, reports, private staging shops and history): its tables, sequences, functions,
-- rows a fresh installation starts with, then constraints, indexes, triggers and row security.
-- Constraints and policies that reference a later component's table live in that component's file.
SET LOCAL check_function_bodies = false;

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

ALTER TABLE ONLY public.entity_history FORCE ROW LEVEL SECURITY;

CREATE SEQUENCE public.entity_history_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE public.entity_history_id_seq OWNED BY public.entity_history.id;

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

ALTER TABLE ONLY public.studio_workspace_versions FORCE ROW LEVEL SECURITY;

CREATE TABLE public.studio_workspaces (
    tenant text NOT NULL,
    user_id text NOT NULL,
    revision bigint NOT NULL,
    data jsonb NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT studio_workspaces_data_check CHECK ((pg_column_size(data) <= 32768)),
    CONSTRAINT studio_workspaces_revision_check CHECK ((revision > 0))
);

ALTER TABLE ONLY public.studio_workspaces FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.entity_history ALTER COLUMN id SET DEFAULT nextval('public.entity_history_id_seq'::regclass);

ALTER TABLE ONLY public.entity_history
    ADD CONSTRAINT entity_history_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.entity_history
    ADD CONSTRAINT entity_history_tenant_entity_entity_id_transaction_id_key UNIQUE (tenant, entity, entity_id, transaction_id);

ALTER TABLE ONLY public.shop_releases
    ADD CONSTRAINT shop_releases_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.studio_workspace_versions
    ADD CONSTRAINT studio_workspace_versions_pkey PRIMARY KEY (tenant, user_id, revision);

ALTER TABLE ONLY public.studio_workspaces
    ADD CONSTRAINT studio_workspaces_pkey PRIMARY KEY (tenant, user_id);

CREATE INDEX entity_history_lookup ON public.entity_history USING btree (tenant, entity, entity_id, id DESC);

ALTER TABLE ONLY public.developer_builds
    ADD CONSTRAINT builds_environment_scope FOREIGN KEY (tenant, environment) REFERENCES public.shop_environments(live_tenant, tenant);

ALTER TABLE ONLY public.developer_builds
    ADD CONSTRAINT developer_builds_environment_fkey FOREIGN KEY (environment) REFERENCES public.shop_environments(tenant);

ALTER TABLE ONLY public.developer_drafts
    ADD CONSTRAINT developer_drafts_environment_tenant_fkey FOREIGN KEY (environment, tenant) REFERENCES public.shop_environments(tenant, live_tenant);

ALTER TABLE ONLY public.shop_releases
    ADD CONSTRAINT releases_environment_scope FOREIGN KEY (live_tenant, environment) REFERENCES public.shop_environments(live_tenant, tenant);

ALTER TABLE ONLY public.shop_environments
    ADD CONSTRAINT shop_environments_preview_owner_fkey FOREIGN KEY (preview_owner) REFERENCES public.merchant_users(id);

ALTER TABLE ONLY public.shop_releases
    ADD CONSTRAINT shop_releases_environment_fkey FOREIGN KEY (environment) REFERENCES public.shop_environments(tenant);

ALTER TABLE ONLY public.studio_workspace_versions
    ADD CONSTRAINT studio_workspace_versions_tenant_user_id_fkey FOREIGN KEY (tenant, user_id) REFERENCES public.studio_workspaces(tenant, user_id) ON DELETE CASCADE;

ALTER TABLE ONLY public.studio_workspaces
    ADD CONSTRAINT studio_workspaces_user_id_tenant_fkey FOREIGN KEY (user_id, tenant) REFERENCES public.memberships(user_id, tenant) ON DELETE CASCADE;

CREATE POLICY authenticated_scope ON public.entity_history AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.studio_workspace_versions AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR ((tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))) AND (user_id = ( SELECT (public.veyndo_scope_guard() ->> 'personalActor'::text)))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR ((tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))) AND (user_id = ( SELECT (public.veyndo_scope_guard() ->> 'personalActor'::text))))));

CREATE POLICY authenticated_scope ON public.studio_workspaces AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR ((tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))) AND (user_id = ( SELECT (public.veyndo_scope_guard() ->> 'personalActor'::text)))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR ((tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))) AND (user_id = ( SELECT (public.veyndo_scope_guard() ->> 'personalActor'::text))))));

CREATE POLICY core_tenant_scope ON public.entity_history USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

ALTER TABLE public.entity_history ENABLE ROW LEVEL SECURITY;

CREATE POLICY personal_workspace ON public.studio_workspaces USING (((tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)) AND (user_id = NULLIF(current_setting('rac.actor'::text, true), ''::text)))) WITH CHECK (((tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)) AND (user_id = NULLIF(current_setting('rac.actor'::text, true), ''::text))));

CREATE POLICY personal_workspace_version ON public.studio_workspace_versions USING (((tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)) AND (user_id = NULLIF(current_setting('rac.actor'::text, true), ''::text)))) WITH CHECK (((tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)) AND (user_id = NULLIF(current_setting('rac.actor'::text, true), ''::text))));

ALTER TABLE public.studio_workspace_versions ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.studio_workspaces ENABLE ROW LEVEL SECURITY;

-- A runtime role that already exists before setup gets its grants here; deploy/sql/provision-runtime.sh
-- grants every table after setup in the usual order.
DO $fn$ BEGIN
 IF EXISTS(SELECT 1 FROM pg_roles WHERE rolname='veyndo_runtime') THEN
  GRANT SELECT,INSERT,UPDATE,DELETE ON studio_workspaces,studio_workspace_versions TO veyndo_runtime;
 END IF;
END $fn$;
