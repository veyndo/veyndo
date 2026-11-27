-- Package contracts (expand). A signed contract travels beside each app Manifest: the effects of its
-- tools for agents, the typed app-to-app ports it provides and consumes, and the data steps of an
-- explicit rollback. The nullable columns are ignored by the previous release; because that release
-- can still install a new version without writing the column, readers only trust a contract whose
-- `version` equals the row's version.
ALTER TABLE public.app_packages ADD COLUMN contract jsonb;
ALTER TABLE public.app_versions ADD COLUMN contract jsonb;
