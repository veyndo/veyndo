-- Baseline schema of the inventory component (Stock, reservations and supply): its tables, sequences, functions,
-- rows a fresh installation starts with, then constraints, indexes, triggers and row security.
-- Constraints and policies that reference a later component's table live in that component's file.
SET LOCAL check_function_bodies = false;

CREATE TABLE public.inventory_reservations (
    tenant text NOT NULL,
    attempt_id text NOT NULL,
    product_id text NOT NULL,
    quantity integer NOT NULL,
    released boolean DEFAULT false NOT NULL,
    CONSTRAINT inventory_reservations_quantity_check CHECK ((quantity > 0))
);

ALTER TABLE ONLY public.inventory_reservations FORCE ROW LEVEL SECURITY;

CREATE TABLE public.order_inventory_reservations (
    tenant text NOT NULL,
    order_id text NOT NULL,
    product_id text NOT NULL,
    quantity integer NOT NULL,
    released_at timestamp with time zone,
    CONSTRAINT order_inventory_reservations_quantity_check CHECK ((quantity > 0))
);

ALTER TABLE ONLY public.order_inventory_reservations FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.inventory_reservations
    ADD CONSTRAINT inventory_reservations_pkey PRIMARY KEY (tenant, attempt_id, product_id);

ALTER TABLE ONLY public.order_inventory_reservations
    ADD CONSTRAINT order_inventory_reservations_pkey PRIMARY KEY (tenant, order_id, product_id);

ALTER TABLE ONLY public.inventory_reservations
    ADD CONSTRAINT inventory_reservations_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.order_inventory_reservations
    ADD CONSTRAINT order_inventory_reservations_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.order_inventory_reservations
    ADD CONSTRAINT order_inventory_reservations_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id);

CREATE POLICY authenticated_scope ON public.inventory_reservations AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.order_inventory_reservations AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY core_tenant_scope ON public.inventory_reservations USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.order_inventory_reservations USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

ALTER TABLE public.inventory_reservations ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.order_inventory_reservations ENABLE ROW LEVEL SECURITY;
