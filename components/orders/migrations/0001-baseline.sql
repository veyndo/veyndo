-- Baseline schema of the orders component (Order state, fulfilment, receipts, documents and merchant order work): its tables, sequences, functions,
-- rows a fresh installation starts with, then constraints, indexes, triggers and row security.
-- Constraints and policies that reference a later component's table live in that component's file.
SET LOCAL check_function_bodies = false;

CREATE FUNCTION public.order_document_accounting() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
DECLARE shop text; target_app text; delta_rows bigint; delta_bytes bigint;
BEGIN
 shop := CASE WHEN TG_OP='DELETE' THEN OLD.tenant ELSE NEW.tenant END;
 target_app := CASE WHEN TG_OP='DELETE' THEN OLD.source_app ELSE NEW.source_app END;
 IF target_app IS NULL THEN RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END; END IF;
 IF TG_OP='UPDATE' THEN RAISE EXCEPTION 'Imported order documents are immutable' USING ERRCODE='42501'; END IF;
 delta_rows := CASE WHEN TG_OP='INSERT' THEN 1 ELSE -1 END;
 delta_bytes := CASE WHEN TG_OP='INSERT' THEN octet_length(NEW.content)+octet_length(NEW.snapshot::text)+512 ELSE -(octet_length(OLD.content)+octet_length(OLD.snapshot::text)+512) END;
 UPDATE app_storage_usage SET rows=rows+delta_rows,bytes=bytes+delta_bytes,updated_at=now() WHERE tenant=shop AND app=target_app;
 IF NOT FOUND THEN RAISE EXCEPTION 'App storage quota is not initialized' USING ERRCODE='42501'; END IF;
 RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END $$;

CREATE TABLE public.order_activity (
    id bigint NOT NULL,
    tenant text NOT NULL,
    order_id text NOT NULL,
    actor text NOT NULL,
    kind text NOT NULL,
    data jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

ALTER TABLE ONLY public.order_activity FORCE ROW LEVEL SECURITY;

CREATE SEQUENCE public.order_activity_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE public.order_activity_id_seq OWNED BY public.order_activity.id;

CREATE TABLE public.order_receipts (
    id text NOT NULL,
    tenant text NOT NULL,
    order_id text NOT NULL,
    kind text NOT NULL,
    number text NOT NULL,
    locale text NOT NULL,
    snapshot jsonb NOT NULL,
    request_key text NOT NULL,
    fingerprint text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    content bytea,
    source_app text,
    customer_visible boolean DEFAULT true NOT NULL,
    CONSTRAINT order_document_size CHECK (((content IS NULL) OR ((octet_length(content) >= 5) AND (octet_length(content) <= 2097152))))
);

ALTER TABLE ONLY public.order_receipts FORCE ROW LEVEL SECURITY;

CREATE TABLE public.order_state_machines (
    tenant text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL
);

ALTER TABLE ONLY public.order_state_machines FORCE ROW LEVEL SECURITY;

CREATE TABLE public.order_transition_requests (
    tenant text NOT NULL,
    request_key text NOT NULL,
    order_id text NOT NULL,
    fingerprint text NOT NULL,
    response jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

ALTER TABLE ONLY public.order_transition_requests FORCE ROW LEVEL SECURITY;

CREATE TABLE public.orders (
    id text NOT NULL,
    tenant text NOT NULL,
    cart_id text NOT NULL,
    idempotency_key text NOT NULL,
    fingerprint text NOT NULL,
    data jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

ALTER TABLE ONLY public.orders FORCE ROW LEVEL SECURITY;

CREATE TABLE public.receipt_counters (
    tenant text NOT NULL,
    kind text NOT NULL,
    next_number bigint DEFAULT 1 NOT NULL
);

ALTER TABLE ONLY public.receipt_counters FORCE ROW LEVEL SECURITY;

CREATE TABLE public.receipt_settings (
    tenant text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL
);

ALTER TABLE ONLY public.receipt_settings FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.order_activity ALTER COLUMN id SET DEFAULT nextval('public.order_activity_id_seq'::regclass);

ALTER TABLE ONLY public.order_activity
    ADD CONSTRAINT order_activity_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.order_receipts
    ADD CONSTRAINT order_receipts_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.order_receipts
    ADD CONSTRAINT order_receipts_tenant_number_key UNIQUE (tenant, number);

ALTER TABLE ONLY public.order_receipts
    ADD CONSTRAINT order_receipts_tenant_request_key_key UNIQUE (tenant, request_key);

ALTER TABLE ONLY public.order_state_machines
    ADD CONSTRAINT order_state_machines_pkey PRIMARY KEY (tenant);

ALTER TABLE ONLY public.order_transition_requests
    ADD CONSTRAINT order_transition_requests_pkey PRIMARY KEY (tenant, request_key);

ALTER TABLE ONLY public.orders
    ADD CONSTRAINT orders_cart_id_key UNIQUE (cart_id);

ALTER TABLE ONLY public.orders
    ADD CONSTRAINT orders_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.orders
    ADD CONSTRAINT orders_tenant_id_key UNIQUE (tenant, id);

ALTER TABLE ONLY public.orders
    ADD CONSTRAINT orders_tenant_idempotency_key_key UNIQUE (tenant, idempotency_key);

ALTER TABLE ONLY public.receipt_counters
    ADD CONSTRAINT receipt_counters_pkey PRIMARY KEY (tenant, kind);

ALTER TABLE ONLY public.receipt_settings
    ADD CONSTRAINT receipt_settings_pkey PRIMARY KEY (tenant);

CREATE INDEX order_activity_scope ON public.order_activity USING btree (tenant, order_id, id);

CREATE INDEX order_chronological_page ON public.orders USING btree (tenant, created_at DESC, id DESC);

CREATE INDEX order_customer_rule_history ON public.orders USING btree (tenant, (((data -> 'orderCustomer'::text) ->> 'customerId'::text)), created_at);

CREATE INDEX order_receipts_scope ON public.order_receipts USING btree (tenant, order_id, created_at);

CREATE INDEX orders_customer_id ON public.orders USING btree (tenant, (((data -> 'orderCustomer'::text) ->> 'customerId'::text)), created_at DESC);

CREATE INDEX orders_guest_contact ON public.orders USING btree (tenant, (((data -> 'orderCustomer'::text) ->> 'email'::text)), created_at DESC, id DESC) WHERE (((data -> 'orderCustomer'::text) ->> 'guest'::text) = 'true'::text);

CREATE INDEX platform_orders_window ON public.orders USING btree (created_at, tenant);

CREATE INDEX tenant_recent_orders ON public.orders USING btree (tenant, created_at DESC, id);

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.orders DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('order', 'id');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.receipt_settings DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('company', 'tenant');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.orders FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('order', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.receipt_settings FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('company', 'tenant');

CREATE TRIGGER order_document_quota AFTER INSERT OR DELETE OR UPDATE ON public.order_receipts FOR EACH ROW EXECUTE FUNCTION public.order_document_accounting();

ALTER TABLE ONLY public.b2b_budget_allocations
    ADD CONSTRAINT b2b_budget_allocations_tenant_order_id_fkey FOREIGN KEY (tenant, order_id) REFERENCES public.orders(tenant, id);

ALTER TABLE ONLY public.order_downloads
    ADD CONSTRAINT downloads_order_scope FOREIGN KEY (tenant, order_id) REFERENCES public.orders(tenant, id);

ALTER TABLE ONLY public.order_activity
    ADD CONSTRAINT order_activity_order_id_fkey FOREIGN KEY (order_id) REFERENCES public.orders(id);

ALTER TABLE ONLY public.order_activity
    ADD CONSTRAINT order_activity_order_scope FOREIGN KEY (tenant, order_id) REFERENCES public.orders(tenant, id);

ALTER TABLE ONLY public.order_downloads
    ADD CONSTRAINT order_downloads_order_id_fkey FOREIGN KEY (order_id) REFERENCES public.orders(id);

ALTER TABLE ONLY public.order_inventory_reservations
    ADD CONSTRAINT order_inventory_reservations_tenant_order_id_fkey FOREIGN KEY (tenant, order_id) REFERENCES public.orders(tenant, id);

ALTER TABLE ONLY public.order_receipts
    ADD CONSTRAINT order_receipts_order_id_fkey FOREIGN KEY (order_id) REFERENCES public.orders(id);

ALTER TABLE ONLY public.order_state_machines
    ADD CONSTRAINT order_state_machines_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.order_transition_requests
    ADD CONSTRAINT order_transition_requests_order_id_fkey FOREIGN KEY (order_id) REFERENCES public.orders(id);

ALTER TABLE ONLY public.order_transition_requests
    ADD CONSTRAINT order_transition_requests_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.promotion_uses
    ADD CONSTRAINT promotion_uses_order_id_fkey FOREIGN KEY (order_id) REFERENCES public.orders(id);

ALTER TABLE ONLY public.promotion_uses
    ADD CONSTRAINT promotion_uses_order_scope FOREIGN KEY (tenant, order_id) REFERENCES public.orders(tenant, id);

ALTER TABLE ONLY public.order_receipts
    ADD CONSTRAINT receipts_order_scope FOREIGN KEY (tenant, order_id) REFERENCES public.orders(tenant, id);

ALTER TABLE ONLY public.order_transition_requests
    ADD CONSTRAINT transition_order_scope FOREIGN KEY (tenant, order_id) REFERENCES public.orders(tenant, id);

CREATE POLICY authenticated_scope ON public.order_activity AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.order_receipts AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.order_state_machines AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.order_transition_requests AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.orders AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.receipt_counters AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.receipt_settings AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY core_tenant_scope ON public.order_activity USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.order_receipts USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.order_state_machines USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.order_transition_requests USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.orders USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.receipt_counters USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.receipt_settings USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

ALTER TABLE public.order_activity ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.order_receipts ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.order_state_machines ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.order_transition_requests ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.orders ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.receipt_counters ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.receipt_settings ENABLE ROW LEVEL SECURITY;
