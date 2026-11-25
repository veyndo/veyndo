-- Baseline schema of the apps component (App packages, managed data, UI surfaces, Wasm extensions and the app builder): its tables, sequences, functions,
-- rows a fresh installation starts with, then constraints, indexes, triggers and row security.
-- Constraints and policies that reference a later component's table live in that component's file.
SET LOCAL check_function_bodies = false;

CREATE FUNCTION public.app_record_prepare() RETURNS trigger
    LANGUAGE plpgsql
    SET search_path TO 'pg_catalog', 'public', 'pg_temp'
    AS $_$
DECLARE model jsonb; f jsonb; v jsonb; normalized jsonb:='{}'; target text; pos integer; relation_bytes bigint:=0; relation_rows bigint:=0;
BEGIN
 IF TG_OP='UPDATE' AND (NEW.tenant,NEW.app,NEW.entity,NEW.id) IS DISTINCT FROM (OLD.tenant,OLD.app,OLD.entity,OLD.id) THEN
  RAISE EXCEPTION 'App record identity cannot change' USING ERRCODE='42501';
 END IF;
 SELECT definition,version INTO STRICT model,NEW.version FROM app_models WHERE tenant=NEW.tenant AND app=NEW.app AND entity=NEW.entity;
 IF EXISTS(SELECT 1 FROM jsonb_object_keys(NEW.fields) k WHERE NOT EXISTS(SELECT 1 FROM jsonb_array_elements(model->'fields') AS declared(field_definition) WHERE declared.field_definition->>'name'=k)) THEN
  RAISE EXCEPTION 'Unknown app field' USING ERRCODE='23514';
 END IF;
 FOR f IN SELECT value FROM jsonb_array_elements(model->'fields') LOOP
  v:=coalesce(NEW.fields->(f->>'name'),'null'::jsonb);
  IF v='null' THEN
   IF coalesce((f->>'required')::boolean,false) THEN RAISE EXCEPTION 'Required app field missing' USING ERRCODE='23502'; END IF;
  ELSIF coalesce((f->>'translatable')::boolean,false) THEN
   IF jsonb_typeof(v)<>'object' THEN RAISE EXCEPTION 'Translation object required' USING ERRCODE='23514'; END IF;
  ELSIF f->>'kind'='integer' THEN
   IF jsonb_typeof(v)<>'number' OR v::text!~'^-?[0-9]+$' THEN RAISE EXCEPTION 'Integer required' USING ERRCODE='23514'; END IF;
   PERFORM (v::text)::bigint;
  ELSIF f->>'kind'='boolean' THEN
   IF jsonb_typeof(v)<>'boolean' THEN RAISE EXCEPTION 'Boolean required' USING ERRCODE='23514'; END IF;
  ELSIF f->>'kind'='relations' THEN
   IF jsonb_typeof(v)<>'array' OR jsonb_array_length(v)>100 THEN RAISE EXCEPTION 'Relation list required' USING ERRCODE='23514'; END IF;
   IF (SELECT count(*)<>count(DISTINCT x) FROM jsonb_array_elements(v) x) THEN RAISE EXCEPTION 'Duplicate app relation' USING ERRCODE='23505'; END IF;
   pos:=0;
   FOR target IN SELECT jsonb_array_elements_text(v) LOOP
    pos:=pos+1;
    relation_bytes:=relation_bytes+octet_length(jsonb_build_object('tenant',NEW.tenant,'source_id',NEW.id,'target_id',target,'position',pos)::text);
    relation_rows:=relation_rows+1;
   END LOOP;
  ELSIF f->>'kind' NOT IN ('json','money','richtext') AND jsonb_typeof(v)<>'string' THEN
   RAISE EXCEPTION 'Text required' USING ERRCODE='23514';
  END IF;
  normalized:=normalized||jsonb_build_object(f->>'name',v);
 END LOOP;
 NEW.fields:=normalized;
 NEW.storage_rows:=1+relation_rows;
 NEW.storage_bytes:=octet_length((normalized||jsonb_build_object('tenant',NEW.tenant,'id',NEW.id,'revision',NEW.revision))::text)+relation_bytes;
 RETURN NEW;
END $_$;

CREATE FUNCTION public.app_record_project() RETURNS trigger
    LANGUAGE plpgsql SECURITY DEFINER
    SET search_path TO 'pg_catalog', 'public', 'pg_temp'
    AS $$
DECLARE f jsonb; v jsonb; target text; pos integer; shop text; package text; delta_rows bigint; delta_bytes bigint;
BEGIN
 shop:=CASE WHEN TG_OP='DELETE' THEN OLD.tenant ELSE NEW.tenant END;
 package:=CASE WHEN TG_OP='DELETE' THEN OLD.app ELSE NEW.app END;
 delta_rows:=(CASE WHEN TG_OP='DELETE' THEN 0 ELSE NEW.storage_rows END)-(CASE WHEN TG_OP='INSERT' THEN 0 ELSE OLD.storage_rows END);
 delta_bytes:=(CASE WHEN TG_OP='DELETE' THEN 0 ELSE NEW.storage_bytes END)-(CASE WHEN TG_OP='INSERT' THEN 0 ELSE OLD.storage_bytes END);
 UPDATE app_storage_usage SET rows=rows+delta_rows,bytes=bytes+delta_bytes,updated_at=now() WHERE tenant=shop AND app=package;
 IF NOT FOUND THEN RAISE EXCEPTION 'App storage quota is not initialized' USING ERRCODE='42501'; END IF;
 IF TG_OP='DELETE' THEN RETURN OLD; END IF;
 DELETE FROM app_record_indexes WHERE tenant=NEW.tenant AND app=NEW.app AND entity=NEW.entity AND record_id=NEW.id;
 DELETE FROM app_record_references WHERE tenant=NEW.tenant AND app=NEW.app AND entity=NEW.entity AND record_id=NEW.id;
 FOR f IN SELECT value FROM app_models m,jsonb_array_elements(m.definition->'fields') WHERE m.tenant=NEW.tenant AND m.app=NEW.app AND m.entity=NEW.entity LOOP
  v:=NEW.fields->(f->>'name');
  IF v='null'::jsonb OR v IS NULL THEN CONTINUE; END IF;
  IF coalesce((f->>'indexed')::boolean,false) OR coalesce((f->>'unique')::boolean,false) THEN
   INSERT INTO app_record_indexes(tenant,app,entity,field,record_id,value,is_unique) VALUES(NEW.tenant,NEW.app,NEW.entity,f->>'name',NEW.id,v,coalesce((f->>'unique')::boolean,false));
  END IF;
  IF f->>'kind'='relations' THEN
   pos:=0;
   FOR target IN SELECT jsonb_array_elements_text(v) LOOP
    pos:=pos+1;
    INSERT INTO app_record_references(tenant,app,entity,record_id,field,position,target_entity,target_id) VALUES(NEW.tenant,NEW.app,NEW.entity,NEW.id,f->>'name',pos,f->>'references',target);
   END LOOP;
  ELSIF f->>'references' IS NOT NULL THEN
   INSERT INTO app_record_references(tenant,app,entity,record_id,field,position,target_entity,target_id) VALUES(NEW.tenant,NEW.app,NEW.entity,NEW.id,f->>'name',1,f->>'references',v#>>'{}');
  ELSIF f->>'coreReference' IS NOT NULL OR f->>'kind' IN ('image','file') THEN
   target:=CASE WHEN f->>'kind' IN ('image','file') THEN 'asset' ELSE f->>'coreReference' END;
   INSERT INTO app_record_references(tenant,app,entity,record_id,field,position,product_id,customer_id,order_id,channel_id,company_id,unit_id,asset_id)
    VALUES(NEW.tenant,NEW.app,NEW.entity,NEW.id,f->>'name',1,
     CASE WHEN target='product' THEN v#>>'{}' END,CASE WHEN target='customer' THEN v#>>'{}' END,
     CASE WHEN target='order' THEN v#>>'{}' END,CASE WHEN target='salesChannel' THEN v#>>'{}' END,
     CASE WHEN target='b2bCompany' THEN v#>>'{}' END,CASE WHEN target='b2bUnit' THEN v#>>'{}' END,
     CASE WHEN target='asset' THEN v#>>'{}' END);
  END IF;
 END LOOP;
 RETURN NEW;
END $$;

CREATE FUNCTION public.app_storage_accounting() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
DECLARE shop text; row_delta bigint; byte_delta bigint; expected_table text;
BEGIN
 shop := CASE WHEN TG_OP='DELETE' THEN OLD.tenant ELSE NEW.tenant END;
 expected_table := 'app_'||substr(encode(sha256(convert_to(shop||':'||TG_ARGV[0],'UTF8')),'hex'),1,24)||'_'||TG_ARGV[1];
 IF TG_TABLE_NAME<>expected_table OR (TG_OP='UPDATE' AND NEW.tenant<>OLD.tenant) THEN
   RAISE EXCEPTION 'App storage tenant/table mismatch' USING ERRCODE='42501';
 END IF;
 row_delta := CASE WHEN TG_OP='INSERT' THEN 1 WHEN TG_OP='DELETE' THEN -1 ELSE 0 END;
 byte_delta := CASE WHEN TG_OP='DELETE' THEN 0 ELSE octet_length(to_jsonb(NEW)::text) END
             - CASE WHEN TG_OP='INSERT' THEN 0 ELSE octet_length(to_jsonb(OLD)::text) END;
 -- UPDATE serializes concurrent writes against this app's quota, independent of other tenants.
 UPDATE app_storage_usage SET rows=rows+row_delta,bytes=bytes+byte_delta,updated_at=now()
 WHERE tenant=shop AND app=TG_ARGV[0];
 IF NOT FOUND THEN RAISE EXCEPTION 'App storage quota is not initialized' USING ERRCODE='42501'; END IF;
 RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END $$;

CREATE FUNCTION public.app_sync_relations() RETURNS trigger
    LANGUAGE plpgsql
    AS $_$
DECLARE links text; values_json jsonb; item text; ordinal bigint;
BEGIN
 links := TG_ARGV[0]; values_json := to_jsonb(NEW)->TG_ARGV[1];
 IF TG_TABLE_NAME<>TG_ARGV[2] OR NEW.tenant<>current_setting('rac.tenant',true) THEN
   RAISE EXCEPTION 'App relation tenant/table mismatch' USING ERRCODE='42501';
 END IF;
 EXECUTE format('DELETE FROM public.%I WHERE tenant=$1 AND source_id=$2',links) USING NEW.tenant,NEW.id;
 IF values_json IS NOT NULL AND values_json<>'null'::jsonb THEN
   IF jsonb_typeof(values_json)<>'array' OR jsonb_array_length(values_json)>100 THEN
     RAISE EXCEPTION 'App relations require at most 100 record IDs' USING ERRCODE='23514';
   END IF;
   FOR item,ordinal IN SELECT value, ordinality FROM jsonb_array_elements_text(values_json) WITH ORDINALITY LOOP
     EXECUTE format('INSERT INTO public.%I(tenant,source_id,target_id,position) VALUES($1,$2,$3,$4)',links)
       USING NEW.tenant,NEW.id,item,ordinal;
   END LOOP;
 END IF;
 RETURN NEW;
END $_$;

CREATE TABLE public.app_action_log (
    id bigint NOT NULL,
    tenant text NOT NULL,
    app text NOT NULL,
    action text NOT NULL,
    actor text NOT NULL,
    status integer NOT NULL,
    duration_ms bigint NOT NULL,
    input_digest text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

ALTER TABLE ONLY public.app_action_log FORCE ROW LEVEL SECURITY;

CREATE SEQUENCE public.app_action_log_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE public.app_action_log_id_seq OWNED BY public.app_action_log.id;

CREATE TABLE public.app_deliveries (
    tenant text NOT NULL,
    app text NOT NULL,
    event_id bigint NOT NULL,
    state text DEFAULT 'queued'::text NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    lease_until timestamp with time zone,
    available_at timestamp with time zone DEFAULT now() NOT NULL,
    error text,
    lease_token uuid,
    last_attempt_at timestamp with time zone
);

ALTER TABLE ONLY public.app_deliveries FORCE ROW LEVEL SECURITY;

CREATE TABLE public.app_evidence (
    tenant text NOT NULL,
    app text NOT NULL,
    source_id text NOT NULL,
    kind text NOT NULL,
    title text NOT NULL,
    body text NOT NULL,
    source_url text NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    digest text NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

ALTER TABLE ONLY public.app_evidence FORCE ROW LEVEL SECURITY;

CREATE TABLE public.app_export_cursors (
    tenant text NOT NULL,
    app text NOT NULL,
    cursor bigint DEFAULT 0 NOT NULL,
    last_poll timestamp with time zone DEFAULT '1970-01-01 00:00:00+00'::timestamp with time zone NOT NULL
);

ALTER TABLE ONLY public.app_export_cursors FORCE ROW LEVEL SECURITY;

CREATE TABLE public.app_jobs (
    tenant text NOT NULL,
    app text NOT NULL,
    id text NOT NULL,
    action text NOT NULL,
    actor text NOT NULL,
    permission text NOT NULL,
    package_digest text NOT NULL,
    request_key text NOT NULL,
    input_digest text NOT NULL,
    input jsonb NOT NULL,
    result jsonb,
    status text DEFAULT 'queued'::text NOT NULL,
    progress integer DEFAULT 0 NOT NULL,
    message jsonb DEFAULT '{}'::jsonb NOT NULL,
    lease text,
    lease_until timestamp with time zone,
    started_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    CONSTRAINT app_jobs_input_check CHECK ((octet_length((input)::text) <= 20000)),
    CONSTRAINT app_jobs_progress_check CHECK (((progress >= 0) AND (progress <= 100))),
    CONSTRAINT app_jobs_result_check CHECK (((result IS NULL) OR (octet_length((result)::text) <= 40000))),
    CONSTRAINT app_jobs_status_check CHECK ((status = ANY (ARRAY['queued'::text, 'running'::text, 'succeeded'::text, 'failed'::text, 'uncertain'::text, 'cancel_requested'::text, 'cancelled'::text])))
);

ALTER TABLE ONLY public.app_jobs FORCE ROW LEVEL SECURITY;

CREATE TABLE public.app_models (
    tenant text NOT NULL,
    app text NOT NULL,
    entity text NOT NULL,
    version text NOT NULL,
    definition jsonb NOT NULL
);

ALTER TABLE ONLY public.app_models FORCE ROW LEVEL SECURITY;

CREATE TABLE public.app_packages (
    tenant text NOT NULL,
    id text NOT NULL,
    version text NOT NULL,
    manifest jsonb NOT NULL,
    digest text NOT NULL,
    active boolean DEFAULT true NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    installed_at timestamp with time zone DEFAULT now() NOT NULL
);

ALTER TABLE ONLY public.app_packages FORCE ROW LEVEL SECURITY;

CREATE TABLE public.app_record_indexes (
    tenant text NOT NULL,
    app text NOT NULL,
    entity text NOT NULL,
    field text NOT NULL,
    record_id text NOT NULL,
    value jsonb NOT NULL,
    value_hash bytea GENERATED ALWAYS AS (sha256(jsonb_send(value))) STORED,
    is_unique boolean NOT NULL
);

ALTER TABLE ONLY public.app_record_indexes FORCE ROW LEVEL SECURITY;

CREATE TABLE public.app_record_references (
    tenant text NOT NULL,
    app text NOT NULL,
    entity text NOT NULL,
    record_id text NOT NULL,
    field text NOT NULL,
    "position" integer NOT NULL,
    target_entity text,
    target_id text,
    product_id text,
    customer_id text,
    order_id text,
    channel_id text,
    company_id text,
    unit_id text,
    asset_id text,
    CONSTRAINT app_record_references_check CHECK ((num_nonnulls(target_id, product_id, customer_id, order_id, channel_id, company_id, unit_id, asset_id) = 1)),
    CONSTRAINT app_record_references_check1 CHECK (((target_entity IS NULL) = (target_id IS NULL))),
    CONSTRAINT app_record_references_position_check CHECK ((("position" >= 1) AND ("position" <= 100)))
);

ALTER TABLE ONLY public.app_record_references FORCE ROW LEVEL SECURITY;

CREATE TABLE public.app_record_versions (
    tenant text NOT NULL,
    app text NOT NULL,
    entity text NOT NULL,
    record_id text NOT NULL,
    revision bigint NOT NULL,
    fields jsonb NOT NULL,
    read_scopes jsonb NOT NULL,
    actor text,
    source text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

ALTER TABLE ONLY public.app_record_versions FORCE ROW LEVEL SECURITY;

CREATE TABLE public.app_records (
    tenant text NOT NULL,
    app text NOT NULL,
    entity text NOT NULL,
    id text NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    version text NOT NULL,
    fields jsonb NOT NULL,
    storage_rows bigint DEFAULT 1 NOT NULL,
    storage_bytes bigint DEFAULT 0 NOT NULL,
    CONSTRAINT app_records_fields_check CHECK ((jsonb_typeof(fields) = 'object'::text)),
    CONSTRAINT app_records_revision_check CHECK ((revision > 0))
);

ALTER TABLE ONLY public.app_records FORCE ROW LEVEL SECURITY;

CREATE TABLE public.app_schedule_runs (
    tenant text NOT NULL,
    app text NOT NULL,
    schedule text NOT NULL,
    due timestamp with time zone NOT NULL,
    event_id bigint NOT NULL
);

ALTER TABLE ONLY public.app_schedule_runs FORCE ROW LEVEL SECURITY;

CREATE TABLE public.app_schedules (
    tenant text NOT NULL,
    app text NOT NULL,
    id text NOT NULL,
    definition jsonb NOT NULL,
    next_run timestamp with time zone NOT NULL
);

ALTER TABLE ONLY public.app_schedules FORCE ROW LEVEL SECURITY;

CREATE TABLE public.app_schema_history (
    tenant text NOT NULL,
    app text NOT NULL,
    to_version text NOT NULL,
    from_version text NOT NULL,
    old_manifest jsonb NOT NULL,
    records jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT app_schema_history_records_check CHECK ((octet_length((records)::text) <= 4194304))
);

ALTER TABLE ONLY public.app_schema_history FORCE ROW LEVEL SECURITY;

CREATE TABLE public.app_service_secrets (
    tenant text NOT NULL,
    app text NOT NULL,
    kind text NOT NULL,
    ciphertext text NOT NULL,
    package_digest text NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    actor text NOT NULL,
    rotated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT app_service_secrets_kind_check CHECK ((kind = ANY (ARRAY['service'::text, 'webhook'::text, 'outbound'::text, 'settings'::text]))),
    CONSTRAINT app_service_secrets_package_digest_check CHECK ((package_digest ~ '^[a-f0-9]{64}$'::text))
);

ALTER TABLE ONLY public.app_service_secrets FORCE ROW LEVEL SECURITY;

CREATE TABLE public.app_storage_usage (
    tenant text NOT NULL,
    app text NOT NULL,
    rows bigint DEFAULT 0 NOT NULL,
    bytes bigint DEFAULT 0 NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT app_storage_usage_bytes_check CHECK (((bytes >= 0) AND (bytes <= 67108864))),
    CONSTRAINT app_storage_usage_rows_check CHECK (((rows >= 0) AND (rows <= 100000)))
);

ALTER TABLE ONLY public.app_storage_usage FORCE ROW LEVEL SECURITY;

CREATE TABLE public.app_surface_grants (
    tenant text NOT NULL,
    token_hash text NOT NULL,
    app text NOT NULL,
    surface text NOT NULL,
    actor_hash text NOT NULL,
    package_digest text NOT NULL,
    actions jsonb NOT NULL,
    context jsonb NOT NULL,
    expires_at timestamp with time zone NOT NULL
);

ALTER TABLE ONLY public.app_surface_grants FORCE ROW LEVEL SECURITY;

CREATE TABLE public.app_versions (
    tenant text NOT NULL,
    app text NOT NULL,
    version text NOT NULL,
    digest text NOT NULL,
    manifest jsonb NOT NULL,
    installed_at timestamp with time zone DEFAULT now() NOT NULL
);

ALTER TABLE ONLY public.app_versions FORCE ROW LEVEL SECURITY;

CREATE TABLE public.app_webhook_receipts (
    tenant text NOT NULL,
    app text NOT NULL,
    webhook text NOT NULL,
    request_key text NOT NULL,
    digest text NOT NULL,
    event_id bigint NOT NULL
);

ALTER TABLE ONLY public.app_webhook_receipts FORCE ROW LEVEL SECURITY;

CREATE TABLE public.developer_builds (
    id text NOT NULL,
    tenant text NOT NULL,
    environment text NOT NULL,
    app text NOT NULL,
    version text NOT NULL,
    digest text NOT NULL,
    manifest jsonb NOT NULL,
    prompt text NOT NULL,
    provider text NOT NULL,
    model text,
    summary jsonb NOT NULL,
    state text DEFAULT 'draft'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    archived boolean DEFAULT false NOT NULL,
    archived_at timestamp with time zone,
    CONSTRAINT developer_builds_state_check CHECK ((state = ANY (ARRAY['draft'::text, 'staged'::text])))
);

ALTER TABLE ONLY public.developer_builds FORCE ROW LEVEL SECURITY;

CREATE TABLE public.developer_drafts (
    tenant text NOT NULL,
    actor_key text NOT NULL,
    id text NOT NULL,
    manifest jsonb NOT NULL,
    environment text,
    revision bigint DEFAULT 1 NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

ALTER TABLE ONLY public.developer_drafts FORCE ROW LEVEL SECURITY;

CREATE TABLE public.extensions (
    tenant text NOT NULL,
    wat text NOT NULL,
    digest text NOT NULL,
    revision bigint DEFAULT 1 NOT NULL
);

ALTER TABLE ONLY public.extensions FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.app_action_log ALTER COLUMN id SET DEFAULT nextval('public.app_action_log_id_seq'::regclass);

REVOKE ALL ON FUNCTION public.app_record_prepare() FROM PUBLIC;

REVOKE ALL ON FUNCTION public.app_record_project() FROM PUBLIC;

ALTER TABLE ONLY public.app_action_log
    ADD CONSTRAINT app_action_log_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.app_deliveries
    ADD CONSTRAINT app_deliveries_pkey PRIMARY KEY (tenant, app, event_id);

ALTER TABLE ONLY public.app_evidence
    ADD CONSTRAINT app_evidence_pkey PRIMARY KEY (tenant, app, source_id);

ALTER TABLE ONLY public.app_export_cursors
    ADD CONSTRAINT app_export_cursors_pkey PRIMARY KEY (tenant, app);

ALTER TABLE ONLY public.app_jobs
    ADD CONSTRAINT app_jobs_pkey PRIMARY KEY (tenant, app, id);

ALTER TABLE ONLY public.app_jobs
    ADD CONSTRAINT app_jobs_tenant_app_request_key_key UNIQUE (tenant, app, request_key);

ALTER TABLE ONLY public.app_models
    ADD CONSTRAINT app_models_pkey PRIMARY KEY (tenant, app, entity);

ALTER TABLE ONLY public.app_packages
    ADD CONSTRAINT app_packages_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.app_record_indexes
    ADD CONSTRAINT app_record_indexes_pkey PRIMARY KEY (tenant, app, entity, field, record_id);

ALTER TABLE ONLY public.app_record_references
    ADD CONSTRAINT app_record_references_pkey PRIMARY KEY (tenant, app, entity, record_id, field, "position");

ALTER TABLE ONLY public.app_record_versions
    ADD CONSTRAINT app_record_versions_pkey PRIMARY KEY (tenant, app, entity, record_id, revision);

ALTER TABLE ONLY public.app_records
    ADD CONSTRAINT app_records_pkey PRIMARY KEY (tenant, app, entity, id);

ALTER TABLE ONLY public.app_schedule_runs
    ADD CONSTRAINT app_schedule_runs_pkey PRIMARY KEY (tenant, app, schedule, due);

ALTER TABLE ONLY public.app_schedules
    ADD CONSTRAINT app_schedules_pkey PRIMARY KEY (tenant, app, id);

ALTER TABLE ONLY public.app_schema_history
    ADD CONSTRAINT app_schema_history_pkey PRIMARY KEY (tenant, app, to_version);

ALTER TABLE ONLY public.app_service_secrets
    ADD CONSTRAINT app_service_secrets_pkey PRIMARY KEY (tenant, app, kind);

ALTER TABLE ONLY public.app_storage_usage
    ADD CONSTRAINT app_storage_usage_pkey PRIMARY KEY (tenant, app);

ALTER TABLE ONLY public.app_surface_grants
    ADD CONSTRAINT app_surface_grants_pkey PRIMARY KEY (tenant, token_hash);

ALTER TABLE ONLY public.app_versions
    ADD CONSTRAINT app_versions_pkey PRIMARY KEY (tenant, app, version);

ALTER TABLE ONLY public.app_webhook_receipts
    ADD CONSTRAINT app_webhook_receipts_pkey PRIMARY KEY (tenant, app, webhook, request_key);

ALTER TABLE ONLY public.developer_builds
    ADD CONSTRAINT developer_builds_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.developer_builds
    ADD CONSTRAINT developer_builds_tenant_app_version_key UNIQUE (tenant, app, version);

ALTER TABLE ONLY public.developer_drafts
    ADD CONSTRAINT developer_drafts_pkey PRIMARY KEY (tenant, actor_key, id);

ALTER TABLE ONLY public.extensions
    ADD CONSTRAINT extensions_pkey PRIMARY KEY (tenant);

CREATE INDEX app_action_log_cursor ON public.app_action_log USING btree (tenant, app, id DESC);

CREATE INDEX app_deliveries_dispatch ON public.app_deliveries USING btree (state, available_at, tenant, app, event_id) WHERE (state = ANY (ARRAY['queued'::text, 'running'::text]));

CREATE INDEX app_deliveries_event_retention ON public.app_deliveries USING btree (event_id);

CREATE INDEX app_deliveries_tenant_running ON public.app_deliveries USING btree (tenant, lease_until) WHERE (state = 'running'::text);

CREATE INDEX app_delivery_fairness ON public.app_deliveries USING btree (tenant, app, last_attempt_at DESC) WHERE (last_attempt_at IS NOT NULL);

CREATE INDEX app_evidence_search ON public.app_evidence USING gin (to_tsvector('simple'::regconfig, ((title || ' '::text) || body)));

CREATE INDEX app_export_poll ON public.app_export_cursors USING btree (last_poll, tenant, app);

CREATE INDEX app_jobs_active ON public.app_jobs USING btree (tenant, app, created_at DESC);

CREATE INDEX app_record_lookup ON public.app_record_indexes USING btree (tenant, app, entity, field, value_hash, record_id);

CREATE INDEX app_record_reference_asset ON public.app_record_references USING btree (tenant, asset_id) WHERE (asset_id IS NOT NULL);

CREATE INDEX app_record_reference_channel ON public.app_record_references USING btree (tenant, channel_id) WHERE (channel_id IS NOT NULL);

CREATE INDEX app_record_reference_company ON public.app_record_references USING btree (tenant, company_id) WHERE (company_id IS NOT NULL);

CREATE INDEX app_record_reference_customer ON public.app_record_references USING btree (tenant, customer_id) WHERE (customer_id IS NOT NULL);

CREATE INDEX app_record_reference_order ON public.app_record_references USING btree (tenant, order_id) WHERE (order_id IS NOT NULL);

CREATE INDEX app_record_reference_product ON public.app_record_references USING btree (tenant, product_id) WHERE (product_id IS NOT NULL);

CREATE INDEX app_record_reference_target ON public.app_record_references USING btree (tenant, app, target_entity, target_id);

CREATE INDEX app_record_reference_unit ON public.app_record_references USING btree (tenant, unit_id) WHERE (unit_id IS NOT NULL);

CREATE UNIQUE INDEX app_record_unique ON public.app_record_indexes USING btree (tenant, app, entity, field, value_hash) WHERE is_unique;

CREATE INDEX app_record_versions_retention ON public.app_record_versions USING btree (tenant, app, created_at);

CREATE INDEX app_schedule_runs_event_retention ON public.app_schedule_runs USING btree (event_id);

CREATE INDEX app_schedules_due ON public.app_schedules USING btree (next_run);

CREATE INDEX app_surface_grant_expiry ON public.app_surface_grants USING btree (tenant, app, expires_at);

CREATE INDEX app_webhook_receipts_event_retention ON public.app_webhook_receipts USING btree (event_id);

CREATE INDEX developer_builds_library ON public.developer_builds USING btree (tenant, archived, created_at DESC);

CREATE TRIGGER app_record_prepare BEFORE INSERT OR UPDATE ON public.app_records FOR EACH ROW EXECUTE FUNCTION public.app_record_prepare();

CREATE TRIGGER app_record_project AFTER INSERT OR DELETE OR UPDATE ON public.app_records FOR EACH ROW EXECUTE FUNCTION public.app_record_project();

CREATE TRIGGER work_ready AFTER INSERT OR UPDATE ON public.app_deliveries FOR EACH ROW EXECUTE FUNCTION public.notify_work_ready();

ALTER TABLE ONLY public.app_action_log
    ADD CONSTRAINT app_action_log_tenant_app_fkey FOREIGN KEY (tenant, app) REFERENCES public.app_packages(tenant, id);

ALTER TABLE ONLY public.app_deliveries
    ADD CONSTRAINT app_deliveries_event_id_fkey FOREIGN KEY (event_id) REFERENCES public.outbox(id);

ALTER TABLE ONLY public.app_deliveries
    ADD CONSTRAINT app_deliveries_event_scope FOREIGN KEY (tenant, event_id) REFERENCES public.outbox(tenant, id);

ALTER TABLE ONLY public.app_deliveries
    ADD CONSTRAINT app_deliveries_tenant_app_fkey FOREIGN KEY (tenant, app) REFERENCES public.app_packages(tenant, id);

ALTER TABLE ONLY public.app_evidence
    ADD CONSTRAINT app_evidence_tenant_app_fkey FOREIGN KEY (tenant, app) REFERENCES public.app_packages(tenant, id);

ALTER TABLE ONLY public.app_evidence
    ADD CONSTRAINT app_evidence_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.app_jobs
    ADD CONSTRAINT app_jobs_tenant_app_fkey FOREIGN KEY (tenant, app) REFERENCES public.app_packages(tenant, id);

ALTER TABLE ONLY public.app_models
    ADD CONSTRAINT app_models_tenant_app_fkey FOREIGN KEY (tenant, app) REFERENCES public.app_packages(tenant, id) DEFERRABLE INITIALLY DEFERRED;

ALTER TABLE ONLY public.app_packages
    ADD CONSTRAINT app_packages_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.app_record_indexes
    ADD CONSTRAINT app_record_indexes_tenant_app_entity_record_id_fkey FOREIGN KEY (tenant, app, entity, record_id) REFERENCES public.app_records(tenant, app, entity, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.app_record_references
    ADD CONSTRAINT app_record_references_tenant_app_entity_record_id_fkey FOREIGN KEY (tenant, app, entity, record_id) REFERENCES public.app_records(tenant, app, entity, id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED;

ALTER TABLE ONLY public.app_record_references
    ADD CONSTRAINT app_record_references_tenant_app_target_entity_target_id_fkey FOREIGN KEY (tenant, app, target_entity, target_id) REFERENCES public.app_records(tenant, app, entity, id) DEFERRABLE INITIALLY DEFERRED;

ALTER TABLE ONLY public.app_record_references
    ADD CONSTRAINT app_record_references_tenant_asset_id_fkey FOREIGN KEY (tenant, asset_id) REFERENCES public.product_assets(tenant, id);

ALTER TABLE ONLY public.app_record_references
    ADD CONSTRAINT app_record_references_tenant_channel_id_fkey FOREIGN KEY (tenant, channel_id) REFERENCES public.sales_channels(tenant, id);

ALTER TABLE ONLY public.app_record_references
    ADD CONSTRAINT app_record_references_tenant_company_id_fkey FOREIGN KEY (tenant, company_id) REFERENCES public.b2b_companies(tenant, id);

ALTER TABLE ONLY public.app_record_references
    ADD CONSTRAINT app_record_references_tenant_customer_id_fkey FOREIGN KEY (tenant, customer_id) REFERENCES public.customers(tenant, id);

ALTER TABLE ONLY public.app_record_references
    ADD CONSTRAINT app_record_references_tenant_order_id_fkey FOREIGN KEY (tenant, order_id) REFERENCES public.orders(tenant, id);

ALTER TABLE ONLY public.app_record_references
    ADD CONSTRAINT app_record_references_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.app_record_references
    ADD CONSTRAINT app_record_references_tenant_unit_id_fkey FOREIGN KEY (tenant, unit_id) REFERENCES public.b2b_units(tenant, id);

ALTER TABLE ONLY public.app_record_versions
    ADD CONSTRAINT app_record_versions_tenant_app_fkey FOREIGN KEY (tenant, app) REFERENCES public.app_packages(tenant, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.app_records
    ADD CONSTRAINT app_records_tenant_app_entity_fkey FOREIGN KEY (tenant, app, entity) REFERENCES public.app_models(tenant, app, entity);

ALTER TABLE ONLY public.app_schedule_runs
    ADD CONSTRAINT app_schedule_runs_event_id_fkey FOREIGN KEY (event_id) REFERENCES public.outbox(id);

ALTER TABLE ONLY public.app_schedules
    ADD CONSTRAINT app_schedules_tenant_app_fkey FOREIGN KEY (tenant, app) REFERENCES public.app_packages(tenant, id);

ALTER TABLE ONLY public.app_schema_history
    ADD CONSTRAINT app_schema_history_tenant_app_fkey FOREIGN KEY (tenant, app) REFERENCES public.app_packages(tenant, id);

ALTER TABLE ONLY public.app_service_secrets
    ADD CONSTRAINT app_service_secrets_tenant_app_fkey FOREIGN KEY (tenant, app) REFERENCES public.app_packages(tenant, id);

ALTER TABLE ONLY public.app_storage_usage
    ADD CONSTRAINT app_storage_usage_tenant_app_fkey FOREIGN KEY (tenant, app) REFERENCES public.app_packages(tenant, id);

ALTER TABLE ONLY public.app_surface_grants
    ADD CONSTRAINT app_surface_grants_tenant_app_fkey FOREIGN KEY (tenant, app) REFERENCES public.app_packages(tenant, id);

ALTER TABLE ONLY public.app_webhook_receipts
    ADD CONSTRAINT app_webhook_receipts_event_id_fkey FOREIGN KEY (event_id) REFERENCES public.outbox(id);

ALTER TABLE ONLY public.developer_builds
    ADD CONSTRAINT developer_builds_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.developer_drafts
    ADD CONSTRAINT developer_drafts_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.hosted_frontends
    ADD CONSTRAINT hosted_frontends_app_fk FOREIGN KEY (tenant, app_id) REFERENCES public.app_packages(tenant, id) ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED;

ALTER TABLE ONLY public.integration_keys
    ADD CONSTRAINT integration_app_scope FOREIGN KEY (tenant, app_id) REFERENCES public.app_packages(tenant, id);

ALTER TABLE ONLY public.order_receipts
    ADD CONSTRAINT order_document_app_scope FOREIGN KEY (tenant, source_app) REFERENCES public.app_packages(tenant, id);

ALTER TABLE ONLY public.payment_provider_accounts
    ADD CONSTRAINT payment_provider_accounts_tenant_app_fkey FOREIGN KEY (tenant, app) REFERENCES public.app_packages(tenant, id);

ALTER TABLE ONLY public.app_schedule_runs
    ADD CONSTRAINT schedule_runs_event_scope FOREIGN KEY (tenant, event_id) REFERENCES public.outbox(tenant, id);

ALTER TABLE ONLY public.app_webhook_receipts
    ADD CONSTRAINT webhook_receipts_event_scope FOREIGN KEY (tenant, event_id) REFERENCES public.outbox(tenant, id);

ALTER TABLE public.app_action_log ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_deliveries ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_evidence ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_export_cursors ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_jobs ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_models ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_packages ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_record_indexes ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_record_references ENABLE ROW LEVEL SECURITY;

CREATE POLICY app_record_version_scope ON public.app_record_versions USING ((tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))) WITH CHECK ((tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)));

ALTER TABLE public.app_record_versions ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_records ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_schedule_runs ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_schedules ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_schema_history ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_service_secrets ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_storage_usage ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_surface_grants ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_versions ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.app_webhook_receipts ENABLE ROW LEVEL SECURITY;

CREATE POLICY authenticated_scope ON public.app_action_log AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_deliveries AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_evidence AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_export_cursors AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_jobs AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_models AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_packages AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_record_indexes AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_record_references AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_record_versions AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_records AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_schedule_runs AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_schedules AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_schema_history AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_service_secrets AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_storage_usage AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_surface_grants AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_versions AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.app_webhook_receipts AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.developer_builds AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.developer_drafts AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.extensions AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY core_tenant_scope ON public.app_deliveries USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.app_evidence USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.app_export_cursors USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.app_models USING (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text))) WITH CHECK (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text)));

CREATE POLICY core_tenant_scope ON public.app_packages USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.app_record_indexes USING (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text))) WITH CHECK (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text)));

CREATE POLICY core_tenant_scope ON public.app_record_references USING (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text))) WITH CHECK (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text)));

CREATE POLICY core_tenant_scope ON public.app_records USING (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text))) WITH CHECK (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text)));

CREATE POLICY core_tenant_scope ON public.app_schedule_runs USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.app_schedules USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.app_versions USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.app_webhook_receipts USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.developer_builds USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.extensions USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

ALTER TABLE public.developer_builds ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.developer_drafts ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.extensions ENABLE ROW LEVEL SECURITY;

CREATE POLICY managed_write_delete ON public.app_models AS RESTRICTIVE FOR DELETE USING (COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false));

CREATE POLICY managed_write_delete ON public.app_record_indexes AS RESTRICTIVE FOR DELETE USING ((pg_trigger_depth() > 0));

CREATE POLICY managed_write_delete ON public.app_record_references AS RESTRICTIVE FOR DELETE USING ((pg_trigger_depth() > 0));

CREATE POLICY managed_write_insert ON public.app_models AS RESTRICTIVE FOR INSERT WITH CHECK (COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false));

CREATE POLICY managed_write_insert ON public.app_record_indexes AS RESTRICTIVE FOR INSERT WITH CHECK ((pg_trigger_depth() > 0));

CREATE POLICY managed_write_insert ON public.app_record_references AS RESTRICTIVE FOR INSERT WITH CHECK ((pg_trigger_depth() > 0));

CREATE POLICY managed_write_update ON public.app_models AS RESTRICTIVE FOR UPDATE USING (COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false)) WITH CHECK (COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false));

CREATE POLICY managed_write_update ON public.app_record_indexes AS RESTRICTIVE FOR UPDATE USING ((pg_trigger_depth() > 0)) WITH CHECK ((pg_trigger_depth() > 0));

CREATE POLICY managed_write_update ON public.app_record_references AS RESTRICTIVE FOR UPDATE USING ((pg_trigger_depth() > 0)) WITH CHECK ((pg_trigger_depth() > 0));

CREATE POLICY tenant_scope ON public.app_action_log USING (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text))) WITH CHECK (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text)));

CREATE POLICY tenant_scope ON public.app_jobs USING ((tenant = current_setting('rac.tenant'::text, true))) WITH CHECK ((tenant = current_setting('rac.tenant'::text, true)));

CREATE POLICY tenant_scope ON public.app_schema_history USING ((tenant = current_setting('rac.tenant'::text, true))) WITH CHECK ((tenant = current_setting('rac.tenant'::text, true)));

CREATE POLICY tenant_scope ON public.app_service_secrets USING (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text))) WITH CHECK (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text)));

CREATE POLICY tenant_scope ON public.app_storage_usage USING (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text))) WITH CHECK (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text)));

CREATE POLICY tenant_scope ON public.app_surface_grants USING (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text))) WITH CHECK (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text)));

CREATE POLICY tenant_scope ON public.developer_drafts USING (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text))) WITH CHECK (((tenant = current_setting('rac.tenant'::text, true)) OR (current_setting('rac.system'::text, true) = 'on'::text)));

-- A runtime role that already exists before setup gets its grants here; deploy/sql/provision-runtime.sh
-- grants every table after setup in the usual order.
DO $fn$ BEGIN
 IF EXISTS(SELECT 1 FROM pg_roles WHERE rolname='veyndo_runtime') THEN
  GRANT SELECT,INSERT,DELETE ON app_record_versions TO veyndo_runtime;
 END IF;
END $fn$;
