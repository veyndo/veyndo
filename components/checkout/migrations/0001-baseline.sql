-- Baseline schema of the checkout component (Cart, quote, checkout transaction and order placement): its tables, sequences, functions,
-- rows a fresh installation starts with, then constraints, indexes, triggers and row security.
-- Constraints and policies that reference a later component's table live in that component's file.
SET LOCAL check_function_bodies = false;

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

ALTER TABLE ONLY public.carts FORCE ROW LEVEL SECURITY;

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

ALTER TABLE ONLY public.checkout_handoffs FORCE ROW LEVEL SECURITY;

CREATE TABLE public.exposures (
    tenant text NOT NULL,
    session text NOT NULL,
    variant text NOT NULL,
    propensity double precision NOT NULL,
    rewarded boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

ALTER TABLE ONLY public.exposures FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.carts
    ADD CONSTRAINT carts_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.carts
    ADD CONSTRAINT carts_tenant_id_key UNIQUE (tenant, id);

ALTER TABLE ONLY public.carts
    ADD CONSTRAINT carts_token_key UNIQUE (token);

ALTER TABLE ONLY public.checkout_handoffs
    ADD CONSTRAINT checkout_handoffs_cart_id_key UNIQUE (cart_id);

ALTER TABLE ONLY public.checkout_handoffs
    ADD CONSTRAINT checkout_handoffs_pkey PRIMARY KEY (digest);

ALTER TABLE ONLY public.exposures
    ADD CONSTRAINT exposures_pkey PRIMARY KEY (tenant, session);

CREATE INDEX carts_sales_channel_lookup ON public.carts USING btree (tenant, sales_channel_id);

CREATE INDEX checkout_handoff_expiry ON public.checkout_handoffs USING btree (expires_at);

CREATE INDEX customer_order_lookup ON public.carts USING btree (tenant, ((data ->> 'email'::text)), id);

CREATE INDEX tenant_exposure_date ON public.exposures USING btree (tenant, created_at);

ALTER TABLE ONLY public.b2b_approvals
    ADD CONSTRAINT b2b_approvals_tenant_cart_id_fkey FOREIGN KEY (tenant, cart_id) REFERENCES public.carts(tenant, id);

ALTER TABLE ONLY public.b2b_quotes
    ADD CONSTRAINT b2b_quotes_tenant_cart_id_fkey FOREIGN KEY (tenant, cart_id) REFERENCES public.carts(tenant, id);

ALTER TABLE ONLY public.carts
    ADD CONSTRAINT carts_sales_channel_tenant_fk FOREIGN KEY (tenant, sales_channel_id) REFERENCES public.sales_channels(tenant, id);

ALTER TABLE ONLY public.checkout_handoffs
    ADD CONSTRAINT checkout_handoffs_cart_id_fkey FOREIGN KEY (cart_id) REFERENCES public.carts(id);

ALTER TABLE ONLY public.checkout_handoffs
    ADD CONSTRAINT checkout_handoffs_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.consumer_requests
    ADD CONSTRAINT consumer_requests_tenant_cart_id_fkey FOREIGN KEY (tenant, cart_id) REFERENCES public.carts(tenant, id);

ALTER TABLE ONLY public.checkout_handoffs
    ADD CONSTRAINT handoffs_cart_scope FOREIGN KEY (tenant, cart_id) REFERENCES public.carts(tenant, id);

ALTER TABLE ONLY public.legal_acceptances
    ADD CONSTRAINT legal_acceptances_tenant_cart_id_fkey FOREIGN KEY (tenant, cart_id) REFERENCES public.carts(tenant, id);

ALTER TABLE ONLY public.orders
    ADD CONSTRAINT orders_cart_id_fkey FOREIGN KEY (cart_id) REFERENCES public.carts(id);

ALTER TABLE ONLY public.orders
    ADD CONSTRAINT orders_cart_scope FOREIGN KEY (tenant, cart_id) REFERENCES public.carts(tenant, id);

ALTER TABLE ONLY public.privacy_consent_log
    ADD CONSTRAINT privacy_consent_log_tenant_cart_id_fkey FOREIGN KEY (tenant, cart_id) REFERENCES public.carts(tenant, id);

ALTER TABLE ONLY public.privacy_consents
    ADD CONSTRAINT privacy_consents_tenant_cart_id_fkey FOREIGN KEY (tenant, cart_id) REFERENCES public.carts(tenant, id);

CREATE POLICY authenticated_scope ON public.carts AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.checkout_handoffs AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.exposures AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

ALTER TABLE public.carts ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.checkout_handoffs ENABLE ROW LEVEL SECURITY;

CREATE POLICY core_tenant_scope ON public.carts USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.checkout_handoffs USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.exposures USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

ALTER TABLE public.exposures ENABLE ROW LEVEL SECURITY;
