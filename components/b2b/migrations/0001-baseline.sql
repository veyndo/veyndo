-- Baseline schema of the b2b component (Company procurement: organisations, budgets, approvals and quotes): its tables, sequences, functions,
-- rows a fresh installation starts with, then constraints, indexes, triggers and row security.
-- Constraints and policies that reference a later component's table live in that component's file.
SET LOCAL check_function_bodies = false;

CREATE TABLE public.b2b_approvals (
    tenant text NOT NULL,
    id text NOT NULL,
    unit_id text NOT NULL,
    buyer_id text NOT NULL,
    cart_id text NOT NULL,
    fingerprint text NOT NULL,
    data jsonb NOT NULL,
    state text DEFAULT 'pending'::text NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT b2b_approvals_revision_check CHECK ((revision > 0)),
    CONSTRAINT b2b_approvals_state_check CHECK ((state = ANY (ARRAY['pending'::text, 'approved'::text, 'rejected'::text, 'consumed'::text])))
);

ALTER TABLE ONLY public.b2b_approvals FORCE ROW LEVEL SECURITY;

CREATE TABLE public.b2b_budget_allocations (
    tenant text NOT NULL,
    order_id text NOT NULL,
    unit_id text NOT NULL,
    period_start date NOT NULL,
    currency text NOT NULL,
    amount_minor bigint NOT NULL,
    released boolean DEFAULT false NOT NULL,
    CONSTRAINT b2b_budget_allocations_amount_minor_check CHECK ((amount_minor >= 0))
);

ALTER TABLE ONLY public.b2b_budget_allocations FORCE ROW LEVEL SECURITY;

CREATE TABLE public.b2b_catalog (
    tenant text NOT NULL,
    unit_id text NOT NULL,
    product_id text NOT NULL
);

ALTER TABLE ONLY public.b2b_catalog FORCE ROW LEVEL SECURITY;

CREATE TABLE public.b2b_companies (
    tenant text NOT NULL,
    id text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT b2b_companies_revision_check CHECK ((revision > 0))
);

ALTER TABLE ONLY public.b2b_companies FORCE ROW LEVEL SECURITY;

CREATE TABLE public.b2b_lists (
    tenant text NOT NULL,
    id text NOT NULL,
    unit_id text NOT NULL,
    owner_id text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    CONSTRAINT b2b_lists_revision_check CHECK ((revision > 0))
);

ALTER TABLE ONLY public.b2b_lists FORCE ROW LEVEL SECURITY;

CREATE TABLE public.b2b_members (
    tenant text NOT NULL,
    unit_id text NOT NULL,
    customer_id text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    CONSTRAINT b2b_members_revision_check CHECK ((revision > 0))
);

ALTER TABLE ONLY public.b2b_members FORCE ROW LEVEL SECURITY;

CREATE TABLE public.b2b_quotes (
    tenant text NOT NULL,
    id text NOT NULL,
    unit_id text NOT NULL,
    buyer_id text NOT NULL,
    cart_id text NOT NULL,
    state text DEFAULT 'requested'::text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT b2b_quotes_revision_check CHECK ((revision > 0)),
    CONSTRAINT b2b_quotes_state_check CHECK (((state = ANY (ARRAY['requested'::text, 'offered'::text, 'accepted'::text, 'ordered'::text, 'rejected'::text])) OR (state ~ '^a[0-9a-f]{8}_[a-z][a-z0-9_]{0,15}$'::text)))
);

ALTER TABLE ONLY public.b2b_quotes FORCE ROW LEVEL SECURITY;

CREATE TABLE public.b2b_units (
    tenant text NOT NULL,
    id text NOT NULL,
    company_id text NOT NULL,
    parent_id text,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    CONSTRAINT b2b_units_check CHECK ((parent_id IS DISTINCT FROM id)),
    CONSTRAINT b2b_units_revision_check CHECK ((revision > 0))
);

ALTER TABLE ONLY public.b2b_units FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.b2b_approvals
    ADD CONSTRAINT b2b_approvals_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.b2b_approvals
    ADD CONSTRAINT b2b_approvals_tenant_cart_id_fingerprint_key UNIQUE (tenant, cart_id, fingerprint);

ALTER TABLE ONLY public.b2b_budget_allocations
    ADD CONSTRAINT b2b_budget_allocations_pkey PRIMARY KEY (tenant, order_id);

ALTER TABLE ONLY public.b2b_catalog
    ADD CONSTRAINT b2b_catalog_pkey PRIMARY KEY (tenant, unit_id, product_id);

ALTER TABLE ONLY public.b2b_companies
    ADD CONSTRAINT b2b_companies_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.b2b_lists
    ADD CONSTRAINT b2b_lists_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.b2b_members
    ADD CONSTRAINT b2b_members_pkey PRIMARY KEY (tenant, unit_id, customer_id);

ALTER TABLE ONLY public.b2b_quotes
    ADD CONSTRAINT b2b_quotes_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.b2b_units
    ADD CONSTRAINT b2b_units_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.b2b_units
    ADD CONSTRAINT b2b_units_tenant_company_id_id_key UNIQUE (tenant, company_id, id);

CREATE INDEX b2b_approval_inbox ON public.b2b_approvals USING btree (tenant, unit_id, state, created_at DESC);

CREATE INDEX b2b_budget_usage ON public.b2b_budget_allocations USING btree (tenant, unit_id, period_start, currency) WHERE (NOT released);

CREATE INDEX b2b_customer_membership ON public.b2b_members USING btree (tenant, customer_id, unit_id);

CREATE INDEX b2b_lists_unit ON public.b2b_lists USING btree (tenant, unit_id, id);

CREATE INDEX b2b_quotes_buyer ON public.b2b_quotes USING btree (tenant, unit_id, buyer_id, created_at DESC);

CREATE TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.b2b_approvals FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bApproval', 'id');

CREATE TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.b2b_catalog FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bUnit', 'unit_id');

CREATE TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.b2b_companies FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bCompany', 'id');

CREATE TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.b2b_lists FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bList', 'id');

CREATE TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.b2b_members FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bUnit', 'unit_id');

CREATE TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.b2b_quotes FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bQuote', 'id');

CREATE TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.b2b_units FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bUnit', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.b2b_approvals FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bApproval', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.b2b_catalog FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bUnit', 'unit_id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.b2b_companies FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bCompany', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.b2b_lists FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bList', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.b2b_members FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bUnit', 'unit_id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.b2b_quotes FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bQuote', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.b2b_units FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('b2bUnit', 'id');

ALTER TABLE ONLY public.b2b_approvals
    ADD CONSTRAINT b2b_approvals_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.b2b_approvals
    ADD CONSTRAINT b2b_approvals_tenant_unit_id_buyer_id_fkey FOREIGN KEY (tenant, unit_id, buyer_id) REFERENCES public.b2b_members(tenant, unit_id, customer_id);

ALTER TABLE ONLY public.b2b_budget_allocations
    ADD CONSTRAINT b2b_budget_allocations_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.b2b_budget_allocations
    ADD CONSTRAINT b2b_budget_allocations_tenant_unit_id_fkey FOREIGN KEY (tenant, unit_id) REFERENCES public.b2b_units(tenant, id);

ALTER TABLE ONLY public.b2b_catalog
    ADD CONSTRAINT b2b_catalog_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.b2b_catalog
    ADD CONSTRAINT b2b_catalog_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.b2b_catalog
    ADD CONSTRAINT b2b_catalog_tenant_unit_id_fkey FOREIGN KEY (tenant, unit_id) REFERENCES public.b2b_units(tenant, id);

ALTER TABLE ONLY public.b2b_companies
    ADD CONSTRAINT b2b_companies_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.b2b_lists
    ADD CONSTRAINT b2b_lists_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.b2b_lists
    ADD CONSTRAINT b2b_lists_tenant_unit_id_owner_id_fkey FOREIGN KEY (tenant, unit_id, owner_id) REFERENCES public.b2b_members(tenant, unit_id, customer_id);

ALTER TABLE ONLY public.b2b_members
    ADD CONSTRAINT b2b_members_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.b2b_members
    ADD CONSTRAINT b2b_members_tenant_unit_id_fkey FOREIGN KEY (tenant, unit_id) REFERENCES public.b2b_units(tenant, id);

ALTER TABLE ONLY public.b2b_quotes
    ADD CONSTRAINT b2b_quotes_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.b2b_quotes
    ADD CONSTRAINT b2b_quotes_tenant_unit_id_buyer_id_fkey FOREIGN KEY (tenant, unit_id, buyer_id) REFERENCES public.b2b_members(tenant, unit_id, customer_id);

ALTER TABLE ONLY public.b2b_units
    ADD CONSTRAINT b2b_units_tenant_company_id_fkey FOREIGN KEY (tenant, company_id) REFERENCES public.b2b_companies(tenant, id);

ALTER TABLE ONLY public.b2b_units
    ADD CONSTRAINT b2b_units_tenant_company_id_parent_id_fkey FOREIGN KEY (tenant, company_id, parent_id) REFERENCES public.b2b_units(tenant, company_id, id);

ALTER TABLE ONLY public.b2b_units
    ADD CONSTRAINT b2b_units_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

CREATE POLICY authenticated_scope ON public.b2b_approvals AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.b2b_budget_allocations AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.b2b_catalog AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.b2b_companies AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.b2b_lists AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.b2b_members AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.b2b_quotes AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.b2b_units AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

ALTER TABLE public.b2b_approvals ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.b2b_budget_allocations ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.b2b_catalog ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.b2b_companies ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.b2b_lists ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.b2b_members ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.b2b_quotes ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.b2b_units ENABLE ROW LEVEL SECURITY;

CREATE POLICY core_tenant_scope ON public.b2b_approvals USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.b2b_budget_allocations USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.b2b_catalog USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.b2b_companies USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.b2b_lists USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.b2b_members USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.b2b_quotes USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.b2b_units USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));
