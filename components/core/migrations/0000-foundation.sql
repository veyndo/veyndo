-- Cross-cutting foundation applied before every component baseline: extensions, the
-- veyndo_authority schema with its per-database key, the signed-scope functions, and the
-- history, notification and scope functions shared by the component tables.
SET LOCAL check_function_bodies = false;

-- BEGIN signed scope: the app-storage host keeps a verbatim copy of this section
-- (extensions/services/app-storage/authority/sql/signed_scope.sql), checked by scripts/component_sources.py.
-- Expand first: existing replicas keep their policies until the signed-pool cutover is verified.
CREATE EXTENSION IF NOT EXISTS pgcrypto;
REVOKE CREATE ON SCHEMA public FROM PUBLIC;
CREATE SCHEMA veyndo_authority;
REVOKE ALL ON SCHEMA veyndo_authority FROM PUBLIC;
CREATE TABLE veyndo_authority.configuration (
 id boolean PRIMARY KEY DEFAULT true CHECK(id), enforced boolean NOT NULL DEFAULT false,
 system_role name, migration_role name NOT NULL, tenant_login name
);
CREATE TABLE veyndo_authority.keys (
 id text PRIMARY KEY, secret bytea NOT NULL CHECK(octet_length(secret)=32),
 active boolean NOT NULL, verify_until timestamptz,
 created_at timestamptz NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX scope_active_key ON veyndo_authority.keys(active) WHERE active;
DO $fn$
DECLARE crypto_schema name;
BEGIN
 INSERT INTO veyndo_authority.configuration(migration_role) VALUES(session_user);
 SELECT n.nspname INTO STRICT crypto_schema FROM pg_extension e JOIN pg_namespace n ON n.oid=e.extnamespace WHERE e.extname='pgcrypto';
 EXECUTE format('CREATE FUNCTION veyndo_authority.mac(payload bytea,secret bytea) RETURNS bytea LANGUAGE sql IMMUTABLE STRICT SET search_path=pg_catalog,pg_temp AS %L',
  format('SELECT %I.hmac(payload,secret,''sha256'')',crypto_schema));
 EXECUTE format('CREATE FUNCTION veyndo_authority.random_key() RETURNS bytea LANGUAGE sql VOLATILE SET search_path=pg_catalog,pg_temp AS %L',
  format('SELECT %I.gen_random_bytes(32)',crypto_schema));
 INSERT INTO veyndo_authority.keys(id,secret,active) VALUES(replace(gen_random_uuid()::text,'-',''),veyndo_authority.random_key(),true);
END;
$fn$;

-- session_user is the actual database login, not this SECURITY DEFINER function's owner.
CREATE FUNCTION public.veyndo_scope_key() RETURNS TABLE(id text,secret text,system_role name,tenant_login name)
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $fn$
DECLARE c veyndo_authority.configuration;
BEGIN
 SELECT * INTO STRICT c FROM veyndo_authority.configuration;
 IF session_user IS DISTINCT FROM c.system_role AND session_user IS DISTINCT FROM c.migration_role THEN
  RAISE EXCEPTION 'Database signing material requires the system connection' USING ERRCODE='42501';
 END IF;
 RETURN QUERY SELECT k.id,encode(k.secret,'hex'),c.system_role,c.tenant_login FROM veyndo_authority.keys k WHERE k.active;
END;
$fn$;

CREATE FUNCTION public.veyndo_authenticated_scope() RETURNS jsonb
LANGUAGE plpgsql STABLE SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $fn$
DECLARE c veyndo_authority.configuration; parts text[]; key_bytes bytea;
 expected bytea; supplied bytea; deadline bigint; epoch_now bigint; body text;
BEGIN
 SELECT * INTO STRICT c FROM veyndo_authority.configuration;
 IF session_user IS NOT DISTINCT FROM c.migration_role THEN
  RETURN jsonb_build_object('system',true,'migration',true,'authenticated',true);
 END IF;
 body:=current_setting('veyndo.scope_ticket',true);
 IF body IS NULL OR length(body)>512 THEN RETURN '{}'::jsonb; END IF;
 parts:=string_to_array(body,':');
 IF cardinality(parts)<>8 OR parts[1]<>'v1' OR parts[2]!~'^[a-f0-9]{32}$'
  OR parts[3]!~'^[0-9]{1,12}$' OR parts[4] NOT IN ('S','T','P','A','D')
  OR parts[5]!~'^[A-Za-z0-9_-]{0,64}$' OR parts[6]!~'^[A-Za-z0-9_-]{0,64}$'
  OR parts[7]!~'^[a-f0-9]{32}$' OR parts[8]!~'^[a-f0-9]{64}$' THEN RETURN '{}'::jsonb; END IF;
 deadline:=parts[3]::bigint; epoch_now:=floor(extract(epoch FROM statement_timestamp()))::bigint;
 IF deadline<epoch_now OR deadline>epoch_now+600 THEN RETURN '{}'::jsonb; END IF;
 SELECT k.secret INTO key_bytes FROM veyndo_authority.keys k
 WHERE k.id=parts[2] AND (k.active OR k.verify_until>statement_timestamp());
 IF key_bytes IS NULL THEN RETURN '{}'::jsonb; END IF;
 body:=array_to_string(parts[1:7],':')||':'||session_user;
 expected:=veyndo_authority.mac(convert_to(body,'UTF8'),key_bytes);
 supplied:=decode(parts[8],'hex');
 -- Blinded comparison does not expose the position of an incorrect MAC byte.
 IF veyndo_authority.mac(expected,key_bytes)<>veyndo_authority.mac(supplied,key_bytes) THEN RETURN '{}'::jsonb; END IF;
 IF parts[4]='S' THEN
  IF session_user IS DISTINCT FROM c.system_role OR parts[5]<>'' OR parts[6]<>'' THEN RETURN '{}'::jsonb; END IF;
  RETURN jsonb_build_object('system',true,'authenticated',true);
 END IF;
 IF session_user<>c.tenant_login OR session_user IS NOT DISTINCT FROM c.system_role THEN RETURN '{}'::jsonb; END IF;
 IF parts[4]='D' THEN RETURN jsonb_build_object('system',false,'authenticated',true); END IF;
 IF parts[5]='' OR parts[4] IN ('A','P') AND parts[6]='' OR parts[4]='T' AND parts[6]<>'' THEN RETURN '{}'::jsonb; END IF;
 RETURN jsonb_build_object('system',false,'authenticated',true,'tenant',parts[5],
  'actor',nullif(parts[6],''),'personalActor',CASE WHEN parts[4]='P' THEN parts[6] ELSE NULL END);
END;
$fn$;

CREATE FUNCTION public.veyndo_scope_guard() RETURNS jsonb
LANGUAGE plpgsql STABLE SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $fn$
BEGIN
 IF NOT (SELECT enforced FROM veyndo_authority.configuration) THEN RETURN '{"system":true}'::jsonb; END IF;
 RETURN public.veyndo_authenticated_scope();
END;
$fn$;
-- END signed scope

-- No COMMENT ON EXTENSION: on managed PostgreSQL the provider owns the extension; CREATE EXTENSION sets the comment.
CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public;

CREATE FUNCTION public.capture_entity_history() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
DECLARE rowdata jsonb;t text;k text;s jsonb;kind text;
BEGIN
 rowdata:=CASE WHEN TG_OP='DELETE' THEN to_jsonb(OLD) ELSE to_jsonb(NEW) END;
 t:=rowdata->>'tenant';kind:=TG_ARGV[0];k:=CASE WHEN TG_ARGV[1]='tenant' THEN 'base' ELSE rowdata->>TG_ARGV[1] END;
 IF TG_WHEN='BEFORE' THEN
  IF NOT EXISTS(SELECT 1 FROM entity_history WHERE tenant=t AND entity=kind AND entity_id=k AND transaction_id=txid_current()) THEN
   s:=entity_history_snapshot(kind,t,k);
   INSERT INTO entity_history(tenant,entity,entity_id,transaction_id,actor,source,reason,before_state)
   VALUES(t,kind,k,txid_current(),nullif(current_setting('veyndo.actor',true),''),coalesce(nullif(current_setting('veyndo.source',true),''),'system'),nullif(current_setting('veyndo.reason',true),''),s);
  END IF;
 ELSIF EXISTS(SELECT 1 FROM entity_history WHERE tenant=t AND entity=kind AND entity_id=k AND transaction_id=txid_current() AND NOT finalized) THEN
  s:=entity_history_snapshot(kind,t,k);
  UPDATE entity_history SET after_state=s,finalized=true WHERE tenant=t AND entity=kind AND entity_id=k AND transaction_id=txid_current();
 END IF;
 RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END $$;

CREATE FUNCTION public.cognitive_source_changed() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
DECLARE object_key text;
BEGIN
 IF TG_TABLE_NAME='products' THEN
  object_key=NEW.id;
  IF TG_OP='UPDATE' AND NEW.name IS NOT DISTINCT FROM OLD.name AND NEW.description IS NOT DISTINCT FROM OLD.description AND NEW.category IS NOT DISTINCT FROM OLD.category THEN RETURN NEW; END IF;
 ELSE
  object_key=NEW.document_id||':'||NEW.position;
  IF TG_OP='UPDATE' AND NEW.text IS NOT DISTINCT FROM OLD.text THEN RETURN NEW; END IF;
 END IF;
 INSERT INTO embedding_jobs(tenant,kind,object_id) VALUES(NEW.tenant,TG_ARGV[0],object_key)
 ON CONFLICT(tenant,kind,object_id) DO UPDATE SET revision=embedding_jobs.revision+1,attempts=0,available_at=now(),lease=NULL,lease_until=NULL,error_code=NULL;
 RETURN NEW;
END $$;

CREATE FUNCTION public.notify_work_ready() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
 IF TG_OP='INSERT' OR coalesce(to_jsonb(NEW)->>'state',to_jsonb(NEW)->>'status')='queued' OR TG_TABLE_NAME='outbox' AND to_jsonb(NEW)->>'delivered_at' IS NULL THEN
  PERFORM pg_notify('veyndo_work_ready',TG_TABLE_NAME);
 END IF;
 RETURN NEW;
END $$;

CREATE FUNCTION public.seed_shop_automation(shop text) RETURNS void
    LANGUAGE plpgsql
    AS $$
DECLARE
 settings jsonb; owner_id text; languages jsonb; main_language text;
 names jsonb; condition jsonb; flow jsonb;
BEGIN
 SELECT data INTO settings FROM commerce_settings WHERE tenant=shop;
 IF settings IS NULL THEN RETURN; END IF;
 languages := coalesce(settings->'locales','["en-GB","de-DE","fr-FR","es-ES"]'::jsonb); main_language := coalesce(settings->>'mainLocale','en-GB');
 SELECT user_id INTO owner_id FROM memberships WHERE tenant=shop AND role='owner' AND active ORDER BY user_id LIMIT 1;
 owner_id := coalesce(owner_id,'bootstrap');
 names := '{"en-GB":"Main storefront","de-DE":"Haupt-Verkaufskanal","es-ES":"Tienda principal","fr-FR":"Boutique principale"}';
 IF NOT names ? main_language THEN names := names || jsonb_build_object(main_language,'Main storefront'); END IF;
 INSERT INTO sales_channels(tenant,id,data) VALUES(shop,'default',jsonb_build_object('name',names,'kind','storefront','active',true,'locales',languages,'productIds','[]'::jsonb,'navigationCategoryId',NULL)) ON CONFLICT DO NOTHING;
 FOR names,condition IN SELECT * FROM (VALUES
 ('{"en-GB":"All orders","de-DE":"Alle Bestellungen","es-ES":"Todos los pedidos","fr-FR":"Toutes les commandes"}'::jsonb,'{"type":"alwaysValid"}'::jsonb),
 ('{"en-GB":"Business customers","de-DE":"Geschäftskunden","es-ES":"Clientes empresariales","fr-FR":"Clients professionnels"}'::jsonb,'{"type":"customerGroup","operator":"=","values":["business"]}'::jsonb),
 ('{"en-GB":"Cart from 100","de-DE":"Warenkorb ab 100","es-ES":"Carrito desde 100","fr-FR":"Panier à partir de 100"}'::jsonb,'{"type":"cartCartAmount","operator":">=","amount":100}'::jsonb)
 ) AS templates(n,c) LOOP
  IF NOT names ? main_language THEN names := names || jsonb_build_object(main_language,names->>'en-GB'); END IF;
  INSERT INTO commerce_rules(tenant,id,name,condition) VALUES(shop,
   CASE condition->>'type' WHEN 'alwaysValid' THEN 'default_all_orders' WHEN 'customerGroup' THEN 'default_business_customers' ELSE 'default_cart_100' END,names,condition) ON CONFLICT DO NOTHING;
 END LOOP;
 FOR flow IN SELECT value FROM jsonb_array_elements('[
 {"id":"default_order_received","event":"order.placed","name":{"en-GB":"Order received","de-DE":"Bestellung eingegangen","es-ES":"Pedido recibido","fr-FR":"Commande reçue"},"instruction":{"en-GB":"Order received. Check fulfillment and payment before dispatch.","de-DE":"Bestellung eingegangen. Lieferung und Zahlung vor dem Versand prüfen.","es-ES":"Pedido recibido. Revisa la entrega y el pago antes del envío.","fr-FR":"Commande reçue. Vérifiez la livraison et le paiement avant expédition."}},
 {"id":"default_payment_received","event":"payment.captured","name":{"en-GB":"Payment received","de-DE":"Zahlung eingegangen","es-ES":"Pago recibido","fr-FR":"Paiement reçu"},"instruction":{"en-GB":"Payment capture confirmed by the payment event.","de-DE":"Zahlungseingang durch das Zahlungsereignis bestätigt.","es-ES":"Cobro confirmado por el evento de pago.","fr-FR":"Encaissement confirmé par l’événement de paiement."}}
 ]'::jsonb) LOOP
  IF NOT (flow->'name') ? main_language THEN
   flow := jsonb_set(flow,'{name}',flow->'name' || jsonb_build_object(main_language,flow->'name'->>'en-GB'));
   flow := jsonb_set(flow,'{instruction}',flow->'instruction' || jsonb_build_object(main_language,flow->'instruction'->>'en-GB'));
  END IF;
  INSERT INTO commerce_flows(tenant,id,data) VALUES(shop,flow->>'id',(flow-'id') || jsonb_build_object('active',true,'condition',jsonb_build_object('type','ruleReference','ruleId','default_all_orders'),'action','note','locale',main_language,'actor',owner_id)) ON CONFLICT DO NOTHING;
 END LOOP;
END $$;

CREATE FUNCTION public.veyndo_activate_scope() RETURNS void
    LANGUAGE plpgsql SECURITY DEFINER
    SET search_path TO 'pg_catalog', 'pg_temp'
    AS $$
DECLARE c veyndo_authority.configuration;
BEGIN
 SELECT * INTO STRICT c FROM veyndo_authority.configuration FOR UPDATE;
 IF session_user IS DISTINCT FROM c.migration_role OR c.tenant_login IS NULL THEN
  RAISE EXCEPTION 'Activation requires configured roles and the migration connection' USING ERRCODE='42501';
 END IF;
 UPDATE veyndo_authority.configuration SET enforced=true;
END;
$$;

CREATE FUNCTION public.veyndo_configure_scope(system_login name, tenant_login name) RETURNS void
    LANGUAGE plpgsql SECURITY DEFINER
    SET search_path TO 'pg_catalog', 'pg_temp'
    AS $$
DECLARE c veyndo_authority.configuration; r record;
BEGIN
 SELECT * INTO STRICT c FROM veyndo_authority.configuration FOR UPDATE;
 IF session_user IS DISTINCT FROM c.migration_role THEN
  RAISE EXCEPTION 'Scope configuration requires the migration connection' USING ERRCODE='42501';
 END IF;
 IF system_login=tenant_login OR system_login=session_user OR tenant_login=session_user
  OR NOT EXISTS(SELECT 1 FROM pg_roles WHERE rolname=system_login AND rolcanlogin AND NOT rolsuper AND NOT rolbypassrls AND NOT rolcreaterole)
  OR NOT EXISTS(SELECT 1 FROM pg_roles WHERE rolname=tenant_login AND rolcanlogin AND NOT rolsuper AND NOT rolbypassrls AND NOT rolcreaterole AND NOT rolcreatedb AND NOT rolreplication)
  OR EXISTS(SELECT 1 FROM pg_roles WHERE rolname<>tenant_login AND pg_has_role(tenant_login,oid,'MEMBER'))
  OR EXISTS(SELECT 1 FROM pg_class WHERE relowner=(SELECT oid FROM pg_roles WHERE rolname=tenant_login)) THEN
  RAISE EXCEPTION 'Authority roles must be distinct restricted logins; tenant login cannot inherit roles or own relations' USING ERRCODE='42501';
 END IF;
 IF c.enforced AND c.tenant_login IS DISTINCT FROM tenant_login THEN
  RAISE EXCEPTION 'Use a coordinated credential cutover before replacing an active tenant login';
 END IF;
 EXECUTE format('GRANT %I TO %I',system_login,session_user);
 EXECUTE format('REVOKE ALL ON SCHEMA public FROM %I',tenant_login);
 EXECUTE format('GRANT USAGE ON SCHEMA public TO %I',tenant_login);
 EXECUTE format('GRANT CONNECT ON DATABASE %I TO %I',current_database(),tenant_login);
 -- PUBLIC's default TEMP grant would let one tenant session shadow native table
 -- names on a reused backend. Revoking a login's own grant cannot remove PUBLIC.
 EXECUTE format('REVOKE TEMPORARY ON DATABASE %I FROM PUBLIC',current_database());
 EXECUTE format('GRANT TEMPORARY ON DATABASE %I TO %I,%I',current_database(),system_login,session_user);
 EXECUTE format('REVOKE CREATE,TEMPORARY ON DATABASE %I FROM %I',current_database(),tenant_login);
 FOR r IN SELECT t.relname FROM pg_class t JOIN pg_namespace n ON n.oid=t.relnamespace
  WHERE n.nspname='public' AND t.relkind IN ('r','v','m','S') LOOP
  EXECUTE format('REVOKE ALL ON public.%I FROM %I',r.relname,tenant_login);
 END LOOP;
 FOR r IN SELECT t.relname FROM pg_class t JOIN pg_namespace n ON n.oid=t.relnamespace
  WHERE n.nspname='public' AND t.relkind='r' AND t.relrowsecurity AND t.relforcerowsecurity
   AND t.relname NOT LIKE 'connector_%'
   AND EXISTS(SELECT 1 FROM pg_attribute a WHERE a.attrelid=t.oid AND a.attname='tenant' AND NOT a.attisdropped) LOOP
  EXECUTE format('GRANT SELECT,INSERT,UPDATE,DELETE,REFERENCES ON public.%I TO %I',r.relname,tenant_login);
 END LOOP;
 EXECUTE format('GRANT SELECT,UPDATE ON public.tenants TO %I',tenant_login);
 EXECUTE format('GRANT SELECT(id,name,email) ON public.merchant_users TO %I',tenant_login);
 EXECUTE format('GRANT SELECT ON public.membership_grants,public.languages,public.read_context_versions,public.commerce_migrations TO %I',tenant_login);
 FOR r IN SELECT t.relname FROM pg_class t JOIN pg_namespace n ON n.oid=t.relnamespace
  WHERE n.nspname='public' AND t.relkind='S' AND t.relname NOT LIKE 'connector_%' LOOP
  EXECUTE format('GRANT USAGE,SELECT ON public.%I TO %I',r.relname,tenant_login);
 END LOOP;
 UPDATE veyndo_authority.configuration SET system_role=system_login,tenant_login=veyndo_configure_scope.tenant_login;
END;
$$;

CREATE FUNCTION public.veyndo_guard_app_table(table_name text) RETURNS void
    LANGUAGE plpgsql SECURITY DEFINER
    SET search_path TO 'pg_catalog', 'pg_temp'
    AS $$
DECLARE c veyndo_authority.configuration; target oid;
BEGIN
 SELECT * INTO STRICT c FROM veyndo_authority.configuration;
 target:=to_regclass(format('public.%I',table_name));
 IF session_user IS DISTINCT FROM c.system_role AND session_user IS DISTINCT FROM c.migration_role
  AND NOT (NOT c.enforced AND EXISTS(SELECT 1 FROM pg_class WHERE oid=target AND pg_has_role(session_user,relowner,'MEMBER'))) THEN
  RAISE EXCEPTION 'App schema changes require the system connection' USING ERRCODE='42501';
 END IF;
 IF table_name!~'^app_[a-f0-9]{16}([a-f0-9]{8})?_' OR target IS NULL
  OR NOT EXISTS(SELECT 1 FROM pg_class WHERE oid=target AND relrowsecurity AND relforcerowsecurity)
  OR NOT EXISTS(SELECT 1 FROM pg_attribute WHERE attrelid=target AND attname='tenant' AND NOT attisdropped) THEN
  RAISE EXCEPTION 'Invalid protected app table' USING ERRCODE='42501';
 END IF;
 IF NOT EXISTS(SELECT 1 FROM pg_policy WHERE polrelid=target AND polname='authenticated_scope') THEN
  EXECUTE format('CREATE POLICY authenticated_scope ON public.%I AS RESTRICTIVE USING (coalesce((SELECT (public.veyndo_scope_guard()->>''system'')::boolean),false) OR tenant=(SELECT public.veyndo_scope_guard()->>''tenant'')) WITH CHECK (coalesce((SELECT (public.veyndo_scope_guard()->>''system'')::boolean),false) OR tenant=(SELECT public.veyndo_scope_guard()->>''tenant''))',table_name);
 END IF;
 IF c.tenant_login IS NOT NULL THEN
  EXECUTE format('GRANT SELECT,INSERT,UPDATE,DELETE,REFERENCES ON public.%I TO %I',table_name,c.tenant_login);
 END IF;
END;
$$;

CREATE FUNCTION public.veyndo_merge_http_counts(a jsonb, b jsonb) RETURNS jsonb
    LANGUAGE sql IMMUTABLE PARALLEL SAFE
    AS $_$
 SELECT coalesce(jsonb_object_agg(key,n),'{}'::jsonb)
 FROM (SELECT key,sum(value::bigint) AS n
       FROM (SELECT * FROM jsonb_each_text(a) UNION ALL SELECT * FROM jsonb_each_text(b)) v
       WHERE key ~ '^[45][0-9]{2}$' GROUP BY key) totals
$_$;

CREATE FUNCTION public.veyndo_register_language(new_locale text) RETURNS void
    LANGUAGE plpgsql SECURITY DEFINER
    SET search_path TO 'pg_catalog', 'pg_temp'
    AS $_$
DECLARE scope jsonb:=public.veyndo_scope_guard();
BEGIN
 IF NOT coalesce((scope->>'system')::boolean,false)
  AND NOT (coalesce((scope->>'authenticated')::boolean,false) AND scope->>'tenant' IS NOT NULL) THEN
  RAISE EXCEPTION 'Language registration requires authenticated scope' USING ERRCODE='42501';
 END IF;
 IF length(new_locale)>35 OR new_locale!~'^[a-z]{2,3}(-[A-Z][a-z]{3})?(-([A-Z]{2}|[0-9]{3}))?(-([a-z0-9]{5,8}|[0-9][a-z0-9]{3}))?$' THEN
  RAISE EXCEPTION 'Invalid canonical language';
 END IF;
 IF EXISTS(SELECT 1 FROM public.languages l WHERE l.locale=new_locale) THEN RETURN; END IF;
 PERFORM pg_advisory_xact_lock(7193512);
 IF (SELECT count(*) FROM public.languages)>=10000 THEN RAISE EXCEPTION 'Language registry capacity reached'; END IF;
 INSERT INTO public.languages(id,locale)
  VALUES(substr(encode(sha256(convert_to('commerce-language:'||new_locale,'UTF8')),'hex'),1,32),new_locale)
  ON CONFLICT(locale) DO NOTHING;
END;
$_$;

CREATE FUNCTION public.veyndo_rotate_scope_key() RETURNS text
    LANGUAGE plpgsql SECURITY DEFINER
    SET search_path TO 'pg_catalog', 'pg_temp'
    AS $$
DECLARE c veyndo_authority.configuration; new_id text:=replace(gen_random_uuid()::text,'-','');
BEGIN
 SELECT * INTO STRICT c FROM veyndo_authority.configuration FOR UPDATE;
 IF session_user IS DISTINCT FROM c.migration_role THEN
  RAISE EXCEPTION 'Scope rotation requires the migration connection' USING ERRCODE='42501';
 END IF;
 DELETE FROM veyndo_authority.keys WHERE NOT active AND verify_until<now();
 UPDATE veyndo_authority.keys SET active=false,verify_until=now()+interval '10 minutes' WHERE active;
 INSERT INTO veyndo_authority.keys(id,secret,active) VALUES(new_id,veyndo_authority.random_key(),true);
 RETURN new_id;
END;
$$;

CREATE FUNCTION public.veyndo_scope_status() RETURNS jsonb
    LANGUAGE sql STABLE SECURITY DEFINER
    SET search_path TO 'pg_catalog', 'pg_temp'
    AS $$
 SELECT jsonb_build_object('enforced',enforced,'tenantRole',tenant_login,'systemRole',system_role)
 FROM veyndo_authority.configuration;
$$;
