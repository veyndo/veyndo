-- Baseline schema of the customers component (Customer accounts, sessions, addresses and the merchant customer directory): its tables, sequences, functions,
-- rows a fresh installation starts with, then constraints, indexes, triggers and row security.
-- Constraints and policies that reference a later component's table live in that component's file.
SET LOCAL check_function_bodies = false;

CREATE TABLE public.customer_addresses (
    tenant text NOT NULL,
    email text NOT NULL,
    id text NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    data jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

ALTER TABLE ONLY public.customer_addresses FORCE ROW LEVEL SECURITY;

CREATE TABLE public.customer_sessions (
    digest text NOT NULL,
    tenant text NOT NULL,
    email text NOT NULL,
    expires_at timestamp with time zone DEFAULT (now() + '7 days'::interval) NOT NULL
);

ALTER TABLE ONLY public.customer_sessions FORCE ROW LEVEL SECURITY;

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

ALTER TABLE ONLY public.customers FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.customer_addresses
    ADD CONSTRAINT customer_addresses_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.customer_addresses
    ADD CONSTRAINT customer_addresses_tenant_email_id_key UNIQUE (tenant, email, id);

ALTER TABLE ONLY public.customer_sessions
    ADD CONSTRAINT customer_sessions_pkey PRIMARY KEY (digest);

ALTER TABLE ONLY public.customers
    ADD CONSTRAINT customers_pkey PRIMARY KEY (tenant, email);

ALTER TABLE ONLY public.customers
    ADD CONSTRAINT customers_tenant_id UNIQUE (tenant, id);

ALTER TABLE ONLY public.customers
    ADD CONSTRAINT customers_tenant_number UNIQUE (tenant, customer_number);

CREATE INDEX customer_addresses_owner ON public.customer_addresses USING btree (tenant, email, id);

CREATE INDEX customer_sessions_expiry ON public.customer_sessions USING btree (expires_at);

CREATE INDEX customers_identity_lookup ON public.customers USING gin (lower(((((((((email || ' '::text) || COALESCE((profile ->> 'name'::text), ''::text)) || ' '::text) || COALESCE((profile ->> 'firstName'::text), ''::text)) || ' '::text) || COALESCE((profile ->> 'lastName'::text), ''::text)) || ' '::text) || COALESCE(customer_number, ''::text))) public.gin_trgm_ops);

CREATE INDEX customers_sales_channel_lookup ON public.customers USING btree (tenant, sales_channel_id);

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.customer_addresses DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('customer', 'email');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.customers DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('customer', 'email');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.customer_addresses FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('customer', 'email');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.customers FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('customer', 'email');

ALTER TABLE ONLY public.b2b_members
    ADD CONSTRAINT b2b_members_tenant_customer_id_fkey FOREIGN KEY (tenant, customer_id) REFERENCES public.customers(tenant, id);

ALTER TABLE ONLY public.customer_addresses
    ADD CONSTRAINT customer_addresses_tenant_email_fkey FOREIGN KEY (tenant, email) REFERENCES public.customers(tenant, email) ON DELETE CASCADE;

ALTER TABLE ONLY public.customer_sessions
    ADD CONSTRAINT customer_sessions_tenant_email_fkey FOREIGN KEY (tenant, email) REFERENCES public.customers(tenant, email) ON DELETE CASCADE;

ALTER TABLE ONLY public.customers
    ADD CONSTRAINT customers_sales_channel_tenant_fk FOREIGN KEY (tenant, sales_channel_id) REFERENCES public.sales_channels(tenant, id);

ALTER TABLE ONLY public.customers
    ADD CONSTRAINT customers_tenant_email_default_billing_address_id_fkey FOREIGN KEY (tenant, email, default_billing_address_id) REFERENCES public.customer_addresses(tenant, email, id) ON DELETE SET NULL (default_billing_address_id);

ALTER TABLE ONLY public.customers
    ADD CONSTRAINT customers_tenant_email_default_shipping_address_id_fkey FOREIGN KEY (tenant, email, default_shipping_address_id) REFERENCES public.customer_addresses(tenant, email, id) ON DELETE SET NULL (default_shipping_address_id);

CREATE POLICY authenticated_scope ON public.customer_addresses AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.customer_sessions AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.customers AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY core_tenant_scope ON public.customer_addresses USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.customer_sessions USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.customers USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

ALTER TABLE public.customer_addresses ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.customer_sessions ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.customers ENABLE ROW LEVEL SECURITY;
