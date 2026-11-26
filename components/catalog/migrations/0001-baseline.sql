-- Baseline schema of the catalog component (Products, categories, media, reviews, translations and catalogue reads): its tables, sequences, functions,
-- rows a fresh installation starts with, then constraints, indexes, triggers and row security.
-- Constraints and policies that reference a later component's table live in that component's file.
SET LOCAL check_function_bodies = false;

CREATE FUNCTION public.knowledge_product_words() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
 IF TG_OP='UPDATE' AND OLD.name=NEW.name AND OLD.description=NEW.description THEN RETURN NULL; END IF;
 DELETE FROM knowledge_product_lexemes WHERE tenant=NEW.tenant AND product_id=NEW.id;
 INSERT INTO knowledge_product_lexemes SELECT NEW.tenant,NEW.id,word
 FROM unnest(tsvector_to_array(to_tsvector('simple',NEW.name||' '||NEW.description))) word;
 RETURN NULL;
END $$;

CREATE FUNCTION public.product_content_clock() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
 IF NEW.revision IS DISTINCT FROM OLD.revision THEN
  IF NEW.stock IS NOT DISTINCT FROM OLD.stock OR
     (to_jsonb(NEW)-ARRAY['stock','revision','content_revision']) IS DISTINCT FROM
     (to_jsonb(OLD)-ARRAY['stock','revision','content_revision']) THEN
   NEW.content_revision:=OLD.content_revision+1;
  ELSE
   NEW.content_revision:=OLD.content_revision;
  END IF;
 END IF;
 RETURN NEW;
END;
$$;

CREATE TABLE public.categories (
    tenant text NOT NULL,
    id text NOT NULL,
    parent_id text,
    "position" integer DEFAULT 0 NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    CONSTRAINT categories_check CHECK (((parent_id IS NULL) OR (parent_id <> id)))
);

ALTER TABLE ONLY public.categories FORCE ROW LEVEL SECURITY;

CREATE TABLE public.media_jobs (
    tenant text NOT NULL,
    id text NOT NULL,
    product_id text NOT NULL,
    product_revision bigint NOT NULL,
    request jsonb NOT NULL,
    state text DEFAULT 'queued'::text NOT NULL,
    asset_id text,
    error text,
    started_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT media_jobs_state_check CHECK ((state = ANY (ARRAY['queued'::text, 'processing'::text, 'ready'::text, 'failed'::text, 'applied'::text])))
);

ALTER TABLE ONLY public.media_jobs FORCE ROW LEVEL SECURITY;

CREATE TABLE public.order_downloads (
    tenant text NOT NULL,
    order_id text NOT NULL,
    asset_id text NOT NULL
);

ALTER TABLE ONLY public.order_downloads FORCE ROW LEVEL SECURITY;

CREATE TABLE public.product_assets (
    tenant text NOT NULL,
    id text NOT NULL,
    product_id text NOT NULL,
    title jsonb NOT NULL,
    filename text NOT NULL,
    mime text NOT NULL,
    kind text NOT NULL,
    public boolean DEFAULT false NOT NULL,
    content bytea NOT NULL,
    digest text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT product_assets_kind_check CHECK ((kind = ANY (ARRAY['attachment'::text, 'download'::text])))
);

ALTER TABLE ONLY public.product_assets FORCE ROW LEVEL SECURITY;

CREATE TABLE public.product_categories (
    tenant text NOT NULL,
    product_id text NOT NULL,
    category_id text NOT NULL
);

ALTER TABLE ONLY public.product_categories FORCE ROW LEVEL SECURITY;

CREATE TABLE public.product_channel_visibility (
    tenant text NOT NULL,
    product_id text NOT NULL,
    channel_id text NOT NULL,
    visible boolean NOT NULL
);

ALTER TABLE ONLY public.product_channel_visibility FORCE ROW LEVEL SECURITY;

CREATE TABLE public.product_reviews (
    id text NOT NULL,
    tenant text NOT NULL,
    product_id text NOT NULL,
    session text NOT NULL,
    author text NOT NULL,
    rating integer NOT NULL,
    title text NOT NULL,
    content text NOT NULL,
    approved boolean DEFAULT false NOT NULL,
    verified boolean DEFAULT false NOT NULL,
    demo boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    source_import jsonb,
    CONSTRAINT product_reviews_author_check CHECK (((length(author) >= 1) AND (length(author) <= 80))),
    CONSTRAINT product_reviews_content_check CHECK (((length(content) >= 1) AND (length(content) <= 3000))),
    CONSTRAINT product_reviews_rating_check CHECK (((rating >= 1) AND (rating <= 5))),
    CONSTRAINT product_reviews_title_check CHECK (((length(title) >= 1) AND (length(title) <= 120))),
    CONSTRAINT review_source_provenance CHECK (((source_import IS NULL) OR ((jsonb_typeof(source_import) = 'object'::text) AND (octet_length((source_import)::text) <= 8192) AND (NOT verified))))
);

ALTER TABLE ONLY public.product_reviews FORCE ROW LEVEL SECURITY;

CREATE TABLE public.product_translations (
    tenant text NOT NULL,
    product_id text NOT NULL,
    language_id text NOT NULL,
    name text,
    description text
);

ALTER TABLE ONLY public.product_translations FORCE ROW LEVEL SECURITY;

CREATE TABLE public.products (
    tenant text NOT NULL,
    id text NOT NULL,
    name text NOT NULL,
    category text NOT NULL,
    description text NOT NULL,
    price double precision NOT NULL,
    tax_rate double precision NOT NULL,
    stock integer NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    list_price double precision,
    regulation_price double precision,
    reference_price jsonb,
    advanced_prices jsonb DEFAULT '[]'::jsonb NOT NULL,
    min_purchase integer DEFAULT 1 NOT NULL,
    purchase_steps integer DEFAULT 1 NOT NULL,
    max_purchase integer,
    parent_id text,
    options jsonb DEFAULT '{}'::jsonb NOT NULL,
    media jsonb DEFAULT '[]'::jsonb NOT NULL,
    properties jsonb DEFAULT '{}'::jsonb NOT NULL,
    delivery_days integer DEFAULT 2 NOT NULL,
    extra jsonb DEFAULT '{}'::jsonb NOT NULL,
    active boolean DEFAULT true NOT NULL,
    product_number text,
    created_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    content_revision bigint DEFAULT 1 NOT NULL,
    CONSTRAINT products_content_revision_check CHECK ((content_revision >= 0)),
    CONSTRAINT products_delivery_days_check CHECK (((delivery_days >= 0) AND (delivery_days <= 365))),
    CONSTRAINT products_max_purchase_check CHECK ((max_purchase > 0)),
    CONSTRAINT products_min_purchase_check CHECK ((min_purchase > 0)),
    CONSTRAINT products_price_check CHECK ((price >= (0)::double precision)),
    CONSTRAINT products_purchase_steps_check CHECK ((purchase_steps > 0)),
    CONSTRAINT products_stock_check CHECK ((stock >= 0))
);

ALTER TABLE ONLY public.products FORCE ROW LEVEL SECURITY;

CREATE TABLE public.translation_items (
    tenant text NOT NULL,
    job_id text NOT NULL,
    product_id text NOT NULL,
    revision bigint NOT NULL,
    source jsonb NOT NULL,
    result jsonb NOT NULL,
    status text DEFAULT 'ready'::text NOT NULL,
    evidence jsonb,
    CONSTRAINT translation_evidence_budget CHECK (((evidence IS NULL) OR ((jsonb_typeof(evidence) = 'object'::text) AND (octet_length((evidence)::text) <= 8192))))
);

ALTER TABLE ONLY public.translation_items FORCE ROW LEVEL SECURITY;

CREATE TABLE public.translation_jobs (
    tenant text NOT NULL,
    id text NOT NULL,
    source_locale text NOT NULL,
    target_locale text NOT NULL,
    choice jsonb NOT NULL,
    overwrite boolean DEFAULT false NOT NULL,
    status text DEFAULT 'queued'::text NOT NULL,
    cursor text DEFAULT ''::text NOT NULL,
    highwater text DEFAULT ''::text NOT NULL,
    total bigint NOT NULL,
    processed bigint DEFAULT 0 NOT NULL,
    lease text,
    lease_until timestamp with time zone,
    error text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    product_ids text[],
    run_task text,
    error_code text,
    CONSTRAINT translation_jobs_error_code_check CHECK (((error_code IS NULL) OR (error_code = 'call_interrupted'::text))),
    -- Written with BETWEEN as in its original definition, so the stored expression tree is identical.
    CONSTRAINT translation_selection_budget CHECK (product_ids IS NULL OR (cardinality(product_ids) BETWEEN 1 AND 1000 AND array_position(product_ids, NULL) IS NULL))
);

ALTER TABLE ONLY public.translation_jobs FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.categories
    ADD CONSTRAINT categories_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.media_jobs
    ADD CONSTRAINT media_jobs_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.order_downloads
    ADD CONSTRAINT order_downloads_pkey PRIMARY KEY (tenant, order_id, asset_id);

ALTER TABLE ONLY public.product_assets
    ADD CONSTRAINT product_assets_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.product_categories
    ADD CONSTRAINT product_categories_pkey PRIMARY KEY (tenant, product_id, category_id);

ALTER TABLE ONLY public.product_channel_visibility
    ADD CONSTRAINT product_channel_visibility_pkey PRIMARY KEY (tenant, product_id, channel_id);

ALTER TABLE ONLY public.product_reviews
    ADD CONSTRAINT product_reviews_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.product_reviews
    ADD CONSTRAINT product_reviews_tenant_product_id_session_key UNIQUE (tenant, product_id, session);

ALTER TABLE ONLY public.product_translations
    ADD CONSTRAINT product_translations_pkey PRIMARY KEY (tenant, product_id, language_id);

ALTER TABLE ONLY public.products
    ADD CONSTRAINT products_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.translation_items
    ADD CONSTRAINT translation_items_pkey PRIMARY KEY (tenant, job_id, product_id);

ALTER TABLE ONLY public.translation_jobs
    ADD CONSTRAINT translation_jobs_pkey PRIMARY KEY (tenant, id);

CREATE INDEX approved_reviews ON public.product_reviews USING btree (tenant, product_id, created_at DESC) WHERE approved;

CREATE INDEX category_children ON public.categories USING btree (tenant, parent_id, "position", id);

CREATE INDEX category_products ON public.product_categories USING btree (tenant, category_id, product_id);

CREATE INDEX media_jobs_queue ON public.media_jobs USING btree (created_at) WHERE (state = 'queued'::text);

CREATE INDEX product_admin_name ON public.products USING gin (lower(name) public.gin_trgm_ops);

CREATE INDEX product_admin_number ON public.products USING gin (lower(product_number) public.gin_trgm_ops);

CREATE INDEX product_admin_translation_name ON public.product_translations USING gin (lower(COALESCE(name, ''::text)) public.gin_trgm_ops);

CREATE INDEX product_assets_scope ON public.product_assets USING btree (tenant, product_id, kind);

CREATE INDEX product_category_page ON public.products USING btree (tenant, category, id) WHERE (parent_id IS NULL);

CREATE INDEX product_channel_hidden ON public.product_channel_visibility USING btree (tenant, channel_id, product_id) WHERE (NOT visible);

CREATE INDEX product_family ON public.products USING btree (tenant, parent_id);

CREATE UNIQUE INDEX product_number_unique ON public.products USING btree (tenant, product_number);

CREATE INDEX product_root_page ON public.products USING btree (tenant, id) WHERE (parent_id IS NULL);

CREATE INDEX product_search_substring ON public.products USING gin (lower(((name || ' '::text) || description)) public.gin_trgm_ops) WHERE (parent_id IS NULL);

CREATE INDEX product_variant_page ON public.products USING btree (tenant, parent_id, id) WHERE (parent_id IS NOT NULL);

CREATE INDEX products_search ON public.products USING gin (to_tsvector('simple'::regconfig, ((name || ' '::text) || description)));

CREATE INDEX review_customer_rule_history ON public.product_reviews USING btree (tenant, session);

CREATE INDEX translation_items_ready ON public.translation_items USING btree (tenant, job_id, product_id) WHERE (status = 'ready'::text);

CREATE INDEX translation_jobs_run ON public.translation_jobs USING btree (tenant, run_task) WHERE (run_task IS NOT NULL);

CREATE INDEX translation_jobs_work ON public.translation_jobs USING btree (created_at) WHERE (status = ANY (ARRAY['queued'::text, 'processing'::text]));

CREATE INDEX translation_search_substring ON public.product_translations USING gin (lower(((COALESCE(name, ''::text) || ' '::text) || COALESCE(description, ''::text))) public.gin_trgm_ops);

CREATE TRIGGER cognitive_product_intake AFTER INSERT OR UPDATE ON public.products FOR EACH ROW EXECUTE FUNCTION public.cognitive_source_changed('product');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.categories DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('category', 'id');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.product_categories DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('product', 'product_id');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.product_channel_visibility DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('product', 'product_id');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.product_translations DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('product', 'product_id');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.products DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('product', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.categories FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('category', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.product_categories FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('product', 'product_id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.product_channel_visibility FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('product', 'product_id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.product_translations FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('product', 'product_id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.products FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('product', 'id');

CREATE TRIGGER knowledge_product_words_changed AFTER INSERT OR UPDATE OF name, description ON public.products FOR EACH ROW EXECUTE FUNCTION public.knowledge_product_words();

CREATE TRIGGER product_content_clock BEFORE UPDATE ON public.products FOR EACH ROW EXECUTE FUNCTION public.product_content_clock();

CREATE TRIGGER work_ready AFTER INSERT OR UPDATE ON public.media_jobs FOR EACH ROW EXECUTE FUNCTION public.notify_work_ready();

CREATE TRIGGER work_ready AFTER INSERT OR UPDATE ON public.translation_jobs FOR EACH ROW EXECUTE FUNCTION public.notify_work_ready();

ALTER TABLE ONLY public.categories
    ADD CONSTRAINT categories_tenant_parent_id_fkey FOREIGN KEY (tenant, parent_id) REFERENCES public.categories(tenant, id);

ALTER TABLE ONLY public.media_jobs
    ADD CONSTRAINT media_jobs_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.order_downloads
    ADD CONSTRAINT order_downloads_tenant_asset_id_fkey FOREIGN KEY (tenant, asset_id) REFERENCES public.product_assets(tenant, id);

ALTER TABLE ONLY public.product_assets
    ADD CONSTRAINT product_assets_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.product_categories
    ADD CONSTRAINT product_categories_tenant_category_id_fkey FOREIGN KEY (tenant, category_id) REFERENCES public.categories(tenant, id);

ALTER TABLE ONLY public.product_categories
    ADD CONSTRAINT product_categories_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.product_channel_visibility
    ADD CONSTRAINT product_channel_visibility_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.product_reviews
    ADD CONSTRAINT product_reviews_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.product_translations
    ADD CONSTRAINT product_translations_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.products
    ADD CONSTRAINT products_parent_scope FOREIGN KEY (tenant, parent_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.translation_items
    ADD CONSTRAINT translation_items_tenant_job_id_fkey FOREIGN KEY (tenant, job_id) REFERENCES public.translation_jobs(tenant, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.translation_jobs
    ADD CONSTRAINT translation_jobs_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

CREATE POLICY authenticated_scope ON public.categories AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.media_jobs AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.order_downloads AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.product_assets AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.product_categories AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.product_channel_visibility AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.product_reviews AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.product_translations AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.products AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.translation_items AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.translation_jobs AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

ALTER TABLE public.categories ENABLE ROW LEVEL SECURITY;

CREATE POLICY core_tenant_scope ON public.categories USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.media_jobs USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.order_downloads USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.product_assets USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.product_categories USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.product_channel_visibility USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.product_reviews USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.product_translations USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.products USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.translation_items USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.translation_jobs USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

ALTER TABLE public.media_jobs ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.order_downloads ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.product_assets ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.product_categories ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.product_channel_visibility ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.product_reviews ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.product_translations ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.products ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.translation_items ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.translation_jobs ENABLE ROW LEVEL SECURITY;
