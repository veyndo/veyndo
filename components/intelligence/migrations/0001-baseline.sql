-- Baseline schema of the intelligence component (Shop memory, knowledge retrieval, agents, proposals and model inference): its tables, sequences, functions,
-- rows a fresh installation starts with, then constraints, indexes, triggers and row security.
-- Constraints and policies that reference a later component's table live in that component's file.
SET LOCAL check_function_bodies = false;

CREATE FUNCTION public.cognitive_error_delta() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
DECLARE old_code text; new_code text; old_shop text; new_shop text;
BEGIN
 IF TG_OP<>'INSERT' THEN old_code=OLD.error_code; old_shop=OLD.tenant; END IF;
 IF TG_OP<>'DELETE' THEN new_code=NEW.error_code; new_shop=NEW.tenant; END IF;
 IF old_code IS NOT DISTINCT FROM new_code AND old_shop IS NOT DISTINCT FROM new_shop THEN RETURN NULL; END IF;
 IF old_code IS NOT NULL THEN
  UPDATE knowledge_index_status SET errors=CASE
   WHEN coalesce((errors->>old_code)::bigint,0)<=1 THEN errors-old_code
   ELSE jsonb_set(errors,ARRAY[old_code],to_jsonb((errors->>old_code)::bigint-1)) END
  WHERE tenant=old_shop;
 END IF;
 IF new_code IS NOT NULL AND EXISTS(SELECT 1 FROM tenants WHERE id=new_shop) THEN
  INSERT INTO knowledge_index_status(tenant) VALUES(new_shop) ON CONFLICT DO NOTHING;
  UPDATE knowledge_index_status SET errors=jsonb_set(errors,ARRAY[new_code],to_jsonb(coalesce((errors->>new_code)::bigint,0)+1))
  WHERE tenant=new_shop;
 END IF;
 RETURN NULL;
END $$;

CREATE FUNCTION public.cognitive_relation_write() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
 IF TG_OP<>'INSERT' THEN
  INSERT INTO knowledge_relation_history VALUES(OLD.tenant,OLD.kind,OLD.source_id,OLD.target_id,OLD.revision,to_jsonb(OLD),OLD.recorded_from,clock_timestamp()) ON CONFLICT DO NOTHING;
 END IF;
 IF TG_OP='DELETE' THEN RETURN OLD; END IF;
 IF TG_OP='UPDATE' THEN NEW.revision=OLD.revision+1;NEW.recorded_from=clock_timestamp(); END IF;
 IF NEW.kind IN ('PAIRS_WITH','CO_PURCHASED') THEN NEW.target_kind='product';
 ELSIF NEW.kind='HAS_DOCUMENT' THEN NEW.target_kind='document';
 ELSIF NEW.kind='REFERENCES_PRODUCT' THEN NEW.source_kind='support';NEW.target_kind='product';
 END IF;
 IF NEW.kind IN ('SERVES','PAIRS_WITH') THEN NEW.state='confirmed'; END IF;
 INSERT INTO knowledge_nodes(tenant,kind,id) VALUES(NEW.tenant,NEW.source_kind,NEW.source_id),(NEW.tenant,NEW.target_kind,NEW.target_id) ON CONFLICT DO NOTHING;
 RETURN NEW;
END $$;

CREATE FUNCTION public.cognitive_status_delta() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
DECLARE shop text; delta integer;
BEGIN
 shop=CASE WHEN TG_OP='DELETE' THEN OLD.tenant ELSE NEW.tenant END;
 delta=CASE WHEN TG_OP='DELETE' THEN -1 ELSE 1 END;
 IF EXISTS(SELECT 1 FROM tenants WHERE id=shop) THEN
  INSERT INTO knowledge_index_status(tenant) VALUES(shop) ON CONFLICT DO NOTHING;
  UPDATE knowledge_index_status SET
   indexed_products=indexed_products+CASE WHEN TG_TABLE_NAME='semantic_products' THEN delta ELSE 0 END,
   pending_jobs=pending_jobs+CASE WHEN TG_TABLE_NAME='embedding_jobs' THEN delta ELSE 0 END
  WHERE tenant=shop;
 END IF;
 RETURN NULL;
END $$;

CREATE FUNCTION public.cognitive_vector_digest() RETURNS trigger
    LANGUAGE plpgsql
    AS $$ BEGIN NEW.vector_digest=md5(NEW.embedding::text); RETURN NEW; END $$;

CREATE FUNCTION public.emit_agent_run_state() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
 IF NEW.run_state IS NOT NULL AND (TG_OP='INSERT' OR NEW.run_state IS DISTINCT FROM OLD.run_state) THEN
  INSERT INTO outbox(tenant,kind,data) VALUES(NEW.tenant,'intelligence.run.changed',
   jsonb_build_object('taskId',NEW.id,'state',NEW.run_state,'revision',NEW.run_revision));
 END IF;
 RETURN NEW;
END $$;

CREATE FUNCTION public.knowledge_chunk_words() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
 IF TG_OP='UPDATE' AND OLD.text=NEW.text THEN RETURN NULL; END IF;
 DELETE FROM knowledge_chunk_lexemes WHERE tenant=NEW.tenant AND document_id=NEW.document_id AND position=NEW.position;
 INSERT INTO knowledge_chunk_lexemes SELECT NEW.tenant,NEW.document_id,NEW.position,word
 FROM unnest(tsvector_to_array(to_tsvector('simple',NEW.text))) word;
 RETURN NULL;
END $$;

CREATE FUNCTION public.queue_vector_index() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
DECLARE row_data jsonb;
BEGIN
 row_data := CASE WHEN TG_OP='DELETE' THEN to_jsonb(OLD) ELSE to_jsonb(NEW) END;
 INSERT INTO vector_index_queue(tenant,kind,object_id) VALUES(row_data->>'tenant',TG_ARGV[0],CASE WHEN TG_ARGV[0]='product' THEN row_data->>'product_id' ELSE (row_data->>'document_id')||':'||(row_data->>'position') END)
 ON CONFLICT(tenant,kind,object_id) DO UPDATE SET updated_at=now(),revision=vector_index_queue.revision+1,lease=NULL,lease_until=NULL,attempts=0,error_code=NULL,available_at=now();
 RETURN NEW;
END $$;

CREATE TABLE public.ai_price_budget (
    tenant text NOT NULL,
    day date NOT NULL,
    product_id text NOT NULL,
    baseline_minor bigint NOT NULL,
    currency text NOT NULL,
    task_id text NOT NULL,
    actor text NOT NULL,
    CONSTRAINT ai_price_budget_baseline_minor_check CHECK ((baseline_minor >= 0))
);

ALTER TABLE ONLY public.ai_price_budget FORCE ROW LEVEL SECURITY;

CREATE TABLE public.chat_messages (
    id bigint NOT NULL,
    tenant text NOT NULL,
    conversation_id text NOT NULL,
    role text NOT NULL,
    content text NOT NULL,
    data jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT chat_messages_role_check CHECK ((role = ANY (ARRAY['user'::text, 'assistant'::text, 'system'::text])))
);

ALTER TABLE ONLY public.chat_messages FORCE ROW LEVEL SECURITY;

CREATE SEQUENCE public.chat_messages_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE public.chat_messages_id_seq OWNED BY public.chat_messages.id;

CREATE TABLE public.conversations (
    tenant text NOT NULL,
    id text NOT NULL,
    title text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    turn_owner text,
    turn_until timestamp with time zone,
    owner_id text
);

ALTER TABLE ONLY public.conversations FORCE ROW LEVEL SECURITY;

CREATE TABLE public.embedding_generations (
    tenant text NOT NULL,
    model text NOT NULL,
    phase text DEFAULT 'product'::text NOT NULL,
    cursor text DEFAULT ''::text NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    model_revision bigint DEFAULT 0 NOT NULL,
    CONSTRAINT embedding_generations_model_check CHECK (((length(model) >= 1) AND (length(model) <= 256))),
    CONSTRAINT embedding_generations_model_revision_check CHECK ((model_revision >= 0)),
    CONSTRAINT embedding_generations_phase_check CHECK ((phase = ANY (ARRAY['product'::text, 'document'::text, 'complete'::text])))
);

ALTER TABLE ONLY public.embedding_generations FORCE ROW LEVEL SECURITY;

CREATE TABLE public.embedding_jobs (
    tenant text NOT NULL,
    kind text NOT NULL,
    object_id text NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    available_at timestamp with time zone DEFAULT now() NOT NULL,
    lease text,
    lease_until timestamp with time zone,
    error_code text,
    CONSTRAINT embedding_jobs_kind_check CHECK ((kind = ANY (ARRAY['product'::text, 'document'::text])))
);

ALTER TABLE ONLY public.embedding_jobs FORCE ROW LEVEL SECURITY;

CREATE TABLE public.experiences (
    tenant text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL
);

ALTER TABLE ONLY public.experiences FORCE ROW LEVEL SECURITY;

CREATE TABLE public.intelligence_assignments (
    tenant text NOT NULL,
    experiment_id text NOT NULL,
    cart_id text NOT NULL,
    arm integer NOT NULL,
    baseline double precision NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT intelligence_assignments_arm_check CHECK ((arm = ANY (ARRAY[0, 1]))),
    CONSTRAINT intelligence_assignments_baseline_check CHECK (((baseline >= (0)::double precision) AND (baseline <= (1)::double precision)))
);

ALTER TABLE ONLY public.intelligence_assignments FORCE ROW LEVEL SECURITY;

CREATE TABLE public.intelligence_experiments (
    tenant text NOT NULL,
    id text NOT NULL,
    channel text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    state text NOT NULL,
    withdrawn integer DEFAULT 0 NOT NULL,
    actor text NOT NULL,
    started_at timestamp with time zone,
    ends_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    final_report jsonb,
    CONSTRAINT intelligence_experiments_state_check CHECK ((state = ANY (ARRAY['draft'::text, 'running'::text, 'stopped'::text, 'completed'::text]))),
    CONSTRAINT intelligence_experiments_withdrawn_check CHECK ((withdrawn >= 0))
);

ALTER TABLE ONLY public.intelligence_experiments FORCE ROW LEVEL SECURITY;

CREATE TABLE public.knowledge_chunk_lexemes (
    tenant text NOT NULL,
    document_id text NOT NULL,
    "position" integer NOT NULL,
    lexeme text NOT NULL
);

ALTER TABLE ONLY public.knowledge_chunk_lexemes FORCE ROW LEVEL SECURITY;

CREATE TABLE public.knowledge_chunks (
    tenant text NOT NULL,
    document_id text NOT NULL,
    "position" integer NOT NULL,
    text text NOT NULL,
    embedding real[],
    embedding_model text,
    locale text DEFAULT ''::text NOT NULL,
    CONSTRAINT cognitive_chunk_geometry CHECK (((embedding IS NULL) OR ((cardinality(embedding) >= 1) AND (cardinality(embedding) <= 8192))))
);

ALTER TABLE ONLY public.knowledge_chunks FORCE ROW LEVEL SECURITY;

CREATE TABLE public.knowledge_documents (
    tenant text NOT NULL,
    id text NOT NULL,
    product_id text,
    title text NOT NULL,
    content_hash text NOT NULL,
    content text NOT NULL,
    visibility text DEFAULT 'private'::text NOT NULL,
    source_type text NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    kind text DEFAULT 'document'::text NOT NULL,
    locale text DEFAULT 'en-GB'::text NOT NULL,
    translations jsonb DEFAULT '{}'::jsonb NOT NULL,
    archived boolean DEFAULT false NOT NULL,
    CONSTRAINT knowledge_documents_visibility_check CHECK ((visibility = ANY (ARRAY['private'::text, 'public'::text])))
);

ALTER TABLE ONLY public.knowledge_documents FORCE ROW LEVEL SECURITY;

CREATE TABLE public.knowledge_hypotheses (
    tenant text NOT NULL,
    id text NOT NULL,
    kind text NOT NULL,
    evidence jsonb NOT NULL,
    state text DEFAULT 'proposed'::text NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT knowledge_hypotheses_state_check CHECK ((state = ANY (ARRAY['proposed'::text, 'dismissed'::text, 'experiment'::text, 'published'::text])))
);

ALTER TABLE ONLY public.knowledge_hypotheses FORCE ROW LEVEL SECURITY;

CREATE TABLE public.knowledge_index_status (
    tenant text NOT NULL,
    indexed_products bigint DEFAULT 0 NOT NULL,
    pending_jobs bigint DEFAULT 0 NOT NULL,
    errors jsonb DEFAULT '{}'::jsonb NOT NULL,
    CONSTRAINT knowledge_index_status_indexed_products_check CHECK ((indexed_products >= 0)),
    CONSTRAINT knowledge_index_status_pending_jobs_check CHECK ((pending_jobs >= 0))
);

ALTER TABLE ONLY public.knowledge_index_status FORCE ROW LEVEL SECURITY;

CREATE TABLE public.knowledge_nodes (
    tenant text NOT NULL,
    kind text NOT NULL,
    id text NOT NULL,
    data jsonb DEFAULT '{}'::jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    CONSTRAINT knowledge_nodes_check CHECK (((length(kind) <= 80) AND (length(id) <= 240)))
);

ALTER TABLE ONLY public.knowledge_nodes FORCE ROW LEVEL SECURITY;

CREATE TABLE public.knowledge_product_lexemes (
    tenant text NOT NULL,
    product_id text NOT NULL,
    lexeme text NOT NULL
);

ALTER TABLE ONLY public.knowledge_product_lexemes FORCE ROW LEVEL SECURITY;

CREATE TABLE public.knowledge_receipts (
    tenant text NOT NULL,
    event_id bigint NOT NULL
);

ALTER TABLE ONLY public.knowledge_receipts FORCE ROW LEVEL SECURITY;

CREATE TABLE public.knowledge_relation_history (
    tenant text NOT NULL,
    kind text NOT NULL,
    source_id text NOT NULL,
    target_id text NOT NULL,
    revision bigint NOT NULL,
    snapshot jsonb NOT NULL,
    recorded_from timestamp with time zone NOT NULL,
    recorded_until timestamp with time zone NOT NULL
);

ALTER TABLE ONLY public.knowledge_relation_history FORCE ROW LEVEL SECURITY;

CREATE TABLE public.knowledge_relations (
    tenant text NOT NULL,
    kind text NOT NULL,
    source_id text NOT NULL,
    target_id text NOT NULL,
    data jsonb DEFAULT '{}'::jsonb NOT NULL,
    source_kind text DEFAULT 'product'::text NOT NULL,
    target_kind text DEFAULT 'intent'::text NOT NULL,
    state text DEFAULT 'evidenced'::text NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    confidence numeric DEFAULT 1 NOT NULL,
    valid_from timestamp with time zone DEFAULT now() NOT NULL,
    valid_until timestamp with time zone,
    recorded_from timestamp with time zone DEFAULT now() NOT NULL,
    actor text DEFAULT 'system'::text NOT NULL,
    CONSTRAINT knowledge_relations_confidence_check CHECK (((confidence >= (0)::numeric) AND (confidence <= (1)::numeric))),
    CONSTRAINT knowledge_relations_state_check CHECK ((state = ANY (ARRAY['proposed'::text, 'evidenced'::text, 'confirmed'::text, 'rejected'::text]))),
    CONSTRAINT knowledge_relations_valid_time CHECK (((valid_until IS NULL) OR (valid_until > valid_from)))
);

ALTER TABLE ONLY public.knowledge_relations FORCE ROW LEVEL SECURITY;

CREATE TABLE public.observed_pairs (
    tenant text NOT NULL,
    left_id text NOT NULL,
    right_id text NOT NULL,
    orders bigint NOT NULL,
    last_event bigint NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT observed_pairs_check CHECK ((left_id < right_id))
);

ALTER TABLE ONLY public.observed_pairs FORCE ROW LEVEL SECURITY;

CREATE TABLE public.pair_evidence (
    tenant text NOT NULL,
    left_id text NOT NULL,
    right_id text NOT NULL,
    order_id text NOT NULL,
    event_id bigint NOT NULL,
    simulated boolean NOT NULL
);

ALTER TABLE ONLY public.pair_evidence FORCE ROW LEVEL SECURITY;

CREATE TABLE public.policy (
    tenant text NOT NULL,
    variant text NOT NULL,
    views bigint DEFAULT 0 NOT NULL,
    purchases bigint DEFAULT 0 NOT NULL
);

ALTER TABLE ONLY public.policy FORCE ROW LEVEL SECURITY;

CREATE TABLE public.private_preferences (
    tenant text NOT NULL,
    cart_id text NOT NULL,
    data jsonb NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

ALTER TABLE ONLY public.private_preferences FORCE ROW LEVEL SECURITY;

CREATE TABLE public.semantic_products (
    tenant text NOT NULL,
    product_id text NOT NULL,
    revision bigint NOT NULL,
    embedding real[] NOT NULL,
    embedding_model text NOT NULL,
    content_hash text NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    vector_digest text,
    CONSTRAINT cognitive_product_geometry CHECK (((cardinality(embedding) >= 1) AND (cardinality(embedding) <= 8192)))
);

ALTER TABLE ONLY public.semantic_products FORCE ROW LEVEL SECURITY;

CREATE TABLE public.session_signal_events (
    tenant text NOT NULL,
    id text NOT NULL,
    session text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    purpose text DEFAULT 'personalization'::text NOT NULL,
    kind text DEFAULT 'signal'::text NOT NULL,
    cart_id text,
    channel_id text,
    product_id text,
    customer_id text,
    dimensions jsonb DEFAULT '{}'::jsonb NOT NULL,
    expires_at timestamp with time zone DEFAULT (now() + '30 days'::interval) NOT NULL,
    CONSTRAINT behavior_dimensions_bound CHECK ((octet_length((dimensions)::text) <= 2048)),
    CONSTRAINT session_signal_events_purpose_check CHECK ((purpose = ANY (ARRAY['personalization'::text, 'analytics'::text])))
);

ALTER TABLE ONLY public.session_signal_events FORCE ROW LEVEL SECURITY;

CREATE TABLE public.session_signals (
    tenant text NOT NULL,
    session text NOT NULL,
    product_id text NOT NULL,
    views integer DEFAULT 0 NOT NULL,
    cart_adds integer DEFAULT 0 NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

ALTER TABLE ONLY public.session_signals FORCE ROW LEVEL SECURITY;

CREATE TABLE public.tasks (
    id text NOT NULL,
    tenant text NOT NULL,
    proposal jsonb NOT NULL,
    applied boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    applied_at timestamp with time zone,
    run_state text,
    run_request jsonb,
    run_actor text,
    run_key text,
    run_fingerprint text,
    run_scopes jsonb,
    run_credential text,
    run_personal boolean DEFAULT false NOT NULL,
    run_checkpoint jsonb DEFAULT '{}'::jsonb NOT NULL,
    run_pending text,
    run_lease text,
    run_until timestamp with time zone,
    run_revision bigint DEFAULT 1 NOT NULL,
    run_error jsonb,
    run_updated timestamp with time zone DEFAULT now() NOT NULL,
    conversation_id text,
    run_trace text DEFAULT COALESCE(NULLIF(current_setting('veyndo.trace_id'::text, true), ''::text), replace((gen_random_uuid())::text, '-'::text, ''::text)) NOT NULL,
    run_after timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT tasks_run_checkpoint_limit CHECK ((octet_length((run_checkpoint)::text) <= 1048576)),
    CONSTRAINT tasks_run_request_limit CHECK ((octet_length((run_request)::text) <= 32768)),
    CONSTRAINT tasks_run_state_check CHECK ((run_state = ANY (ARRAY['queued'::text, 'running'::text, 'ready'::text, 'failed'::text, 'uncertain'::text, 'cancelled'::text]))),
    CONSTRAINT tasks_run_trace_check CHECK ((run_trace ~ '^[a-f0-9]{32}$'::text))
);

ALTER TABLE ONLY public.tasks FORCE ROW LEVEL SECURITY;

CREATE TABLE public.tenant_ai_usage (
    tenant text NOT NULL,
    day date NOT NULL,
    attempts bigint NOT NULL,
    CONSTRAINT tenant_ai_usage_attempts_check CHECK ((attempts > 0))
);

ALTER TABLE ONLY public.tenant_ai_usage FORCE ROW LEVEL SECURITY;

CREATE TABLE public.vector_index_queue (
    tenant text NOT NULL,
    kind text NOT NULL,
    object_id text NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    lease text,
    lease_until timestamp with time zone,
    attempts integer DEFAULT 0 NOT NULL,
    available_at timestamp with time zone DEFAULT now() NOT NULL,
    error_code text
);

ALTER TABLE ONLY public.vector_index_queue FORCE ROW LEVEL SECURITY;

ALTER TABLE ONLY public.chat_messages ALTER COLUMN id SET DEFAULT nextval('public.chat_messages_id_seq'::regclass);

-- Rows every fresh installation starts with.
INSERT INTO public.knowledge_index_status (tenant, indexed_products, pending_jobs, errors) VALUES
    ('atelier', '0', '0', '{}'),
    ('workshop', '0', '0', '{}');

ALTER TABLE ONLY public.ai_price_budget
    ADD CONSTRAINT ai_price_budget_pkey PRIMARY KEY (tenant, day, product_id);

ALTER TABLE ONLY public.chat_messages
    ADD CONSTRAINT chat_messages_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.conversations
    ADD CONSTRAINT conversations_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.embedding_generations
    ADD CONSTRAINT embedding_generations_pkey PRIMARY KEY (tenant);

ALTER TABLE ONLY public.embedding_jobs
    ADD CONSTRAINT embedding_jobs_pkey PRIMARY KEY (tenant, kind, object_id);

ALTER TABLE ONLY public.experiences
    ADD CONSTRAINT experiences_pkey PRIMARY KEY (tenant);

ALTER TABLE ONLY public.intelligence_assignments
    ADD CONSTRAINT intelligence_assignments_pkey PRIMARY KEY (tenant, experiment_id, cart_id);

ALTER TABLE ONLY public.intelligence_experiments
    ADD CONSTRAINT intelligence_experiments_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.knowledge_chunk_lexemes
    ADD CONSTRAINT knowledge_chunk_lexemes_pkey PRIMARY KEY (tenant, lexeme, document_id, "position");

ALTER TABLE ONLY public.knowledge_chunks
    ADD CONSTRAINT knowledge_chunks_pkey PRIMARY KEY (tenant, document_id, "position");

ALTER TABLE ONLY public.knowledge_documents
    ADD CONSTRAINT knowledge_documents_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.knowledge_documents
    ADD CONSTRAINT knowledge_documents_tenant_product_id_content_hash_key UNIQUE NULLS NOT DISTINCT (tenant, product_id, content_hash);

ALTER TABLE ONLY public.knowledge_hypotheses
    ADD CONSTRAINT knowledge_hypotheses_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.knowledge_index_status
    ADD CONSTRAINT knowledge_index_status_pkey PRIMARY KEY (tenant);

ALTER TABLE ONLY public.knowledge_nodes
    ADD CONSTRAINT knowledge_nodes_pkey PRIMARY KEY (tenant, kind, id);

ALTER TABLE ONLY public.knowledge_product_lexemes
    ADD CONSTRAINT knowledge_product_lexemes_pkey PRIMARY KEY (tenant, lexeme, product_id);

ALTER TABLE ONLY public.knowledge_receipts
    ADD CONSTRAINT knowledge_receipts_pkey PRIMARY KEY (tenant, event_id);

ALTER TABLE ONLY public.knowledge_relation_history
    ADD CONSTRAINT knowledge_relation_history_pkey PRIMARY KEY (tenant, kind, source_id, target_id, revision);

ALTER TABLE ONLY public.knowledge_relations
    ADD CONSTRAINT knowledge_relations_pkey PRIMARY KEY (tenant, kind, source_id, target_id);

ALTER TABLE ONLY public.observed_pairs
    ADD CONSTRAINT observed_pairs_pkey PRIMARY KEY (tenant, left_id, right_id);

ALTER TABLE ONLY public.pair_evidence
    ADD CONSTRAINT pair_evidence_pkey PRIMARY KEY (tenant, left_id, right_id, order_id);

ALTER TABLE ONLY public.policy
    ADD CONSTRAINT policy_pkey PRIMARY KEY (tenant, variant);

ALTER TABLE ONLY public.private_preferences
    ADD CONSTRAINT private_preferences_pkey PRIMARY KEY (tenant, cart_id);

ALTER TABLE ONLY public.semantic_products
    ADD CONSTRAINT semantic_products_pkey PRIMARY KEY (tenant, product_id);

ALTER TABLE ONLY public.session_signal_events
    ADD CONSTRAINT session_signal_events_pkey PRIMARY KEY (tenant, id);

ALTER TABLE ONLY public.session_signals
    ADD CONSTRAINT session_signals_pkey PRIMARY KEY (tenant, session, product_id);

ALTER TABLE ONLY public.tasks
    ADD CONSTRAINT tasks_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.tasks
    ADD CONSTRAINT tasks_tenant_id_key UNIQUE (tenant, id);

ALTER TABLE ONLY public.tenant_ai_usage
    ADD CONSTRAINT tenant_ai_usage_pkey PRIMARY KEY (tenant, day);

ALTER TABLE ONLY public.vector_index_queue
    ADD CONSTRAINT vector_index_queue_pkey PRIMARY KEY (tenant, kind, object_id);

CREATE INDEX behavior_expiry ON public.session_signal_events USING btree (expires_at) WHERE (purpose = 'analytics'::text);

CREATE INDEX behavior_journey ON public.session_signal_events USING btree (tenant, session, created_at DESC) WHERE (purpose = 'analytics'::text);

CREATE INDEX behavior_window ON public.session_signal_events USING btree (tenant, purpose, created_at DESC) WHERE (purpose = 'analytics'::text);

CREATE INDEX chat_history ON public.chat_messages USING btree (tenant, conversation_id, id);

CREATE INDEX cognitive_chunk_keyset ON public.knowledge_chunks USING btree (tenant, (((document_id || ':'::text) || "position")));

CREATE INDEX conversations_actor ON public.conversations USING btree (tenant, owner_id, created_at DESC);

CREATE INDEX embedding_generations_pending ON public.embedding_generations USING btree (model, tenant) WHERE (phase <> 'complete'::text);

CREATE INDEX embedding_jobs_diagnostics ON public.embedding_jobs USING btree (tenant, error_code) WHERE (error_code IS NOT NULL);

CREATE INDEX embedding_jobs_ready ON public.embedding_jobs USING btree (available_at, tenant) WHERE (attempts < 8);

CREATE INDEX intelligence_assignments_time ON public.intelligence_assignments USING btree (tenant, experiment_id, created_at);

CREATE UNIQUE INDEX intelligence_one_experiment ON public.intelligence_experiments USING btree (tenant, channel) WHERE (state = 'running'::text);

CREATE INDEX knowledge_chunk_lexemes_source ON public.knowledge_chunk_lexemes USING btree (tenant, document_id, "position");

CREATE INDEX knowledge_chunk_search ON public.knowledge_chunks USING gin (to_tsvector('simple'::regconfig, text));

CREATE INDEX knowledge_chunks_language ON public.knowledge_chunks USING btree (tenant, document_id, locale);

CREATE INDEX knowledge_claim_lookup ON public.knowledge_relations USING btree (tenant, source_id, state) WHERE (kind = 'CLAIMS'::text);

CREATE INDEX knowledge_confirmed_query_text ON public.knowledge_relations USING gin (lower((data ->> 'text'::text)) public.gin_trgm_ops) WHERE ((kind = 'CLAIMS'::text) AND (state = 'confirmed'::text));

CREATE INDEX knowledge_confirmed_query_type ON public.knowledge_relations USING btree (tenant, target_kind, confidence) WHERE ((kind = 'CLAIMS'::text) AND (state = 'confirmed'::text));

CREATE INDEX knowledge_documents_workspace ON public.knowledge_documents USING btree (tenant, archived, id);

CREATE INDEX knowledge_product_lexemes_source ON public.knowledge_product_lexemes USING btree (tenant, product_id);

CREATE INDEX knowledge_receipts_event_retention ON public.knowledge_receipts USING btree (event_id);

CREATE INDEX knowledge_relations_target ON public.knowledge_relations USING btree (tenant, kind, target_id);

CREATE INDEX knowledge_serves_target ON public.knowledge_relations USING btree (tenant, target_id, source_id) WHERE (kind = 'SERVES'::text);

CREATE INDEX observed_pairs_event_retention ON public.observed_pairs USING btree (last_event);

CREATE INDEX pair_evidence_event_retention ON public.pair_evidence USING btree (event_id);

CREATE INDEX tasks_actor_recent ON public.tasks USING btree (tenant, run_actor, created_at DESC);

CREATE UNIQUE INDEX tasks_one_conversation_run ON public.tasks USING btree (tenant, conversation_id) WHERE ((conversation_id IS NOT NULL) AND (run_state = ANY (ARRAY['queued'::text, 'running'::text, 'uncertain'::text])));

CREATE INDEX tasks_run_claim ON public.tasks USING btree (run_updated, tenant, id) WHERE (run_state = ANY (ARRAY['queued'::text, 'running'::text]));

CREATE UNIQUE INDEX tasks_run_request_key ON public.tasks USING btree (tenant, run_actor, COALESCE((run_request ->> 'agentId'::text), ''::text), run_key) WHERE (run_key IS NOT NULL);

CREATE INDEX tasks_tenant_active_run ON public.tasks USING btree (tenant, run_state) WHERE (run_state = ANY (ARRAY['queued'::text, 'running'::text, 'uncertain'::text]));

CREATE TRIGGER cognitive_document_intake AFTER INSERT OR UPDATE ON public.knowledge_chunks FOR EACH ROW EXECUTE FUNCTION public.cognitive_source_changed('document');

CREATE TRIGGER cognitive_relation_write BEFORE INSERT OR DELETE OR UPDATE ON public.knowledge_relations FOR EACH ROW EXECUTE FUNCTION public.cognitive_relation_write();

CREATE TRIGGER cognitive_status_errors AFTER INSERT OR DELETE OR UPDATE OF error_code, tenant ON public.embedding_jobs FOR EACH ROW EXECUTE FUNCTION public.cognitive_error_delta();

CREATE TRIGGER cognitive_status_index AFTER INSERT OR DELETE ON public.semantic_products FOR EACH ROW EXECUTE FUNCTION public.cognitive_status_delta();

CREATE TRIGGER cognitive_status_pending AFTER INSERT OR DELETE ON public.embedding_jobs FOR EACH ROW EXECUTE FUNCTION public.cognitive_status_delta();

CREATE TRIGGER cognitive_vector_digest BEFORE INSERT OR UPDATE OF embedding ON public.semantic_products FOR EACH ROW EXECUTE FUNCTION public.cognitive_vector_digest();

CREATE TRIGGER document_index_outbox AFTER INSERT OR DELETE OR UPDATE ON public.knowledge_chunks FOR EACH ROW EXECUTE FUNCTION public.queue_vector_index('document');

CREATE CONSTRAINT TRIGGER history_after AFTER INSERT OR DELETE OR UPDATE ON public.knowledge_documents DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('source', 'id');

CREATE TRIGGER history_before BEFORE INSERT OR DELETE OR UPDATE ON public.knowledge_documents FOR EACH ROW EXECUTE FUNCTION public.capture_entity_history('source', 'id');

CREATE TRIGGER knowledge_chunk_words_changed AFTER INSERT OR UPDATE OF text ON public.knowledge_chunks FOR EACH ROW EXECUTE FUNCTION public.knowledge_chunk_words();

CREATE TRIGGER semantic_index_outbox AFTER INSERT OR DELETE OR UPDATE ON public.semantic_products FOR EACH ROW EXECUTE FUNCTION public.queue_vector_index('product');

CREATE TRIGGER tasks_run_progress AFTER INSERT OR UPDATE OF run_state ON public.tasks FOR EACH ROW EXECUTE FUNCTION public.emit_agent_run_state();

ALTER TABLE ONLY public.ai_price_budget
    ADD CONSTRAINT ai_price_budget_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.ai_price_budget
    ADD CONSTRAINT ai_price_budget_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.ai_price_budget
    ADD CONSTRAINT ai_price_budget_tenant_task_id_fkey FOREIGN KEY (tenant, task_id) REFERENCES public.tasks(tenant, id);

ALTER TABLE ONLY public.session_signal_events
    ADD CONSTRAINT behavior_cart_fk FOREIGN KEY (tenant, cart_id) REFERENCES public.carts(tenant, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.session_signal_events
    ADD CONSTRAINT behavior_channel_fk FOREIGN KEY (tenant, channel_id) REFERENCES public.sales_channels(tenant, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.session_signal_events
    ADD CONSTRAINT behavior_customer_fk FOREIGN KEY (tenant, customer_id) REFERENCES public.customers(tenant, id) ON DELETE SET NULL (customer_id);

ALTER TABLE ONLY public.session_signal_events
    ADD CONSTRAINT behavior_product_fk FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id) ON DELETE SET NULL (product_id);

ALTER TABLE ONLY public.chat_messages
    ADD CONSTRAINT chat_messages_tenant_conversation_id_fkey FOREIGN KEY (tenant, conversation_id) REFERENCES public.conversations(tenant, id);

ALTER TABLE ONLY public.embedding_generations
    ADD CONSTRAINT embedding_generations_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.embedding_jobs
    ADD CONSTRAINT embedding_jobs_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.intelligence_assignments
    ADD CONSTRAINT intelligence_assignments_tenant_cart_id_fkey FOREIGN KEY (tenant, cart_id) REFERENCES public.carts(tenant, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.intelligence_assignments
    ADD CONSTRAINT intelligence_assignments_tenant_experiment_id_fkey FOREIGN KEY (tenant, experiment_id) REFERENCES public.intelligence_experiments(tenant, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.intelligence_experiments
    ADD CONSTRAINT intelligence_experiments_tenant_channel_fkey FOREIGN KEY (tenant, channel) REFERENCES public.sales_channels(tenant, id);

ALTER TABLE ONLY public.intelligence_experiments
    ADD CONSTRAINT intelligence_experiments_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.knowledge_chunk_lexemes
    ADD CONSTRAINT knowledge_chunk_lexemes_tenant_document_id_position_fkey FOREIGN KEY (tenant, document_id, "position") REFERENCES public.knowledge_chunks(tenant, document_id, "position") ON DELETE CASCADE;

ALTER TABLE ONLY public.knowledge_chunks
    ADD CONSTRAINT knowledge_chunks_tenant_document_id_fkey FOREIGN KEY (tenant, document_id) REFERENCES public.knowledge_documents(tenant, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.knowledge_documents
    ADD CONSTRAINT knowledge_documents_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.knowledge_index_status
    ADD CONSTRAINT knowledge_index_status_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.knowledge_nodes
    ADD CONSTRAINT knowledge_nodes_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.knowledge_product_lexemes
    ADD CONSTRAINT knowledge_product_lexemes_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.knowledge_receipts
    ADD CONSTRAINT knowledge_receipts_event_id_fkey FOREIGN KEY (event_id) REFERENCES public.outbox(id);

ALTER TABLE ONLY public.knowledge_receipts
    ADD CONSTRAINT knowledge_receipts_event_scope FOREIGN KEY (tenant, event_id) REFERENCES public.outbox(tenant, id);

ALTER TABLE ONLY public.knowledge_relation_history
    ADD CONSTRAINT knowledge_relation_history_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.knowledge_relations
    ADD CONSTRAINT knowledge_relations_source_node FOREIGN KEY (tenant, source_kind, source_id) REFERENCES public.knowledge_nodes(tenant, kind, id);

ALTER TABLE ONLY public.knowledge_relations
    ADD CONSTRAINT knowledge_relations_target_node FOREIGN KEY (tenant, target_kind, target_id) REFERENCES public.knowledge_nodes(tenant, kind, id);

ALTER TABLE ONLY public.knowledge_relations
    ADD CONSTRAINT knowledge_relations_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.observed_pairs
    ADD CONSTRAINT observed_pairs_event_scope FOREIGN KEY (tenant, last_event) REFERENCES public.outbox(tenant, id);

ALTER TABLE ONLY public.observed_pairs
    ADD CONSTRAINT observed_pairs_last_event_fkey FOREIGN KEY (last_event) REFERENCES public.outbox(id);

ALTER TABLE ONLY public.pair_evidence
    ADD CONSTRAINT pair_evidence_event_id_fkey FOREIGN KEY (event_id) REFERENCES public.outbox(id);

ALTER TABLE ONLY public.pair_evidence
    ADD CONSTRAINT pair_evidence_event_scope FOREIGN KEY (tenant, event_id) REFERENCES public.outbox(tenant, id);

ALTER TABLE ONLY public.private_preferences
    ADD CONSTRAINT private_preferences_tenant_cart_id_fkey FOREIGN KEY (tenant, cart_id) REFERENCES public.carts(tenant, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.semantic_products
    ADD CONSTRAINT semantic_products_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.session_signals
    ADD CONSTRAINT session_signals_tenant_product_id_fkey FOREIGN KEY (tenant, product_id) REFERENCES public.products(tenant, id);

ALTER TABLE ONLY public.tasks
    ADD CONSTRAINT tasks_conversation_scope FOREIGN KEY (tenant, conversation_id) REFERENCES public.conversations(tenant, id);

ALTER TABLE ONLY public.tenant_ai_usage
    ADD CONSTRAINT tenant_ai_usage_tenant_fkey FOREIGN KEY (tenant) REFERENCES public.tenants(id);

ALTER TABLE ONLY public.translation_jobs
    ADD CONSTRAINT translation_run_tenant_fk FOREIGN KEY (tenant, run_task) REFERENCES public.tasks(tenant, id);

ALTER TABLE public.ai_price_budget ENABLE ROW LEVEL SECURITY;

CREATE POLICY authenticated_scope ON public.ai_price_budget AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.chat_messages AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.conversations AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.embedding_generations AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.embedding_jobs AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.experiences AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.intelligence_assignments AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.intelligence_experiments AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.knowledge_chunk_lexemes AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.knowledge_chunks AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.knowledge_documents AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.knowledge_hypotheses AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.knowledge_index_status AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.knowledge_nodes AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.knowledge_product_lexemes AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.knowledge_receipts AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.knowledge_relation_history AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.knowledge_relations AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.observed_pairs AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.pair_evidence AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.policy AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.private_preferences AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.semantic_products AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.session_signal_events AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.session_signals AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.tasks AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.tenant_ai_usage AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

CREATE POLICY authenticated_scope ON public.vector_index_queue AS RESTRICTIVE USING ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text))))) WITH CHECK ((COALESCE(( SELECT ((public.veyndo_scope_guard() ->> 'system'::text))::boolean AS bool), false) OR (tenant = ( SELECT (public.veyndo_scope_guard() ->> 'tenant'::text)))));

ALTER TABLE public.chat_messages ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.conversations ENABLE ROW LEVEL SECURITY;

CREATE POLICY core_tenant_scope ON public.ai_price_budget USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.chat_messages USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.conversations USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.embedding_generations USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.embedding_jobs USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.experiences USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.intelligence_assignments USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.intelligence_experiments USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.knowledge_chunk_lexemes USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.knowledge_chunks USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.knowledge_documents USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.knowledge_hypotheses USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.knowledge_index_status USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.knowledge_nodes USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.knowledge_product_lexemes USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.knowledge_receipts USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.knowledge_relation_history USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.knowledge_relations USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.observed_pairs USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.pair_evidence USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.policy USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.private_preferences USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.semantic_products USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.session_signal_events USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.session_signals USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.tasks USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.tenant_ai_usage USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

CREATE POLICY core_tenant_scope ON public.vector_index_queue USING (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text)))) WITH CHECK (((current_setting('rac.system'::text, true) = 'on'::text) OR (tenant = NULLIF(current_setting('rac.tenant'::text, true), ''::text))));

ALTER TABLE public.embedding_generations ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.embedding_jobs ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.experiences ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.intelligence_assignments ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.intelligence_experiments ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.knowledge_chunk_lexemes ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.knowledge_chunks ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.knowledge_documents ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.knowledge_hypotheses ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.knowledge_index_status ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.knowledge_nodes ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.knowledge_product_lexemes ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.knowledge_receipts ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.knowledge_relation_history ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.knowledge_relations ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.observed_pairs ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.pair_evidence ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.policy ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.private_preferences ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.semantic_products ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.session_signal_events ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.session_signals ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.tasks ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.tenant_ai_usage ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.vector_index_queue ENABLE ROW LEVEL SECURITY;
