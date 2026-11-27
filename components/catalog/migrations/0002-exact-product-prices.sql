-- veyndo:no-transaction
-- Exact product prices and tax rates (expand). The baseline stores `price`, `tax_rate`,
-- `list_price` and `regulation_price` as binary64 `double precision`; this adds exact
-- `numeric` twins that the release reads and writes, and keeps both representations in step
-- for the previous release, which still reads and writes the binary64 columns.
--
-- * `numeric` without a fixed scale: a product price is the decimal the merchant entered in
--   the shop's main currency, before the quote rounds it to that currency's minor unit, so
--   neither a fixed column scale nor integer minor units of one exponent hold every row.
-- * The trigger runs first among the BEFORE triggers (by name), so history and content
--   clocks see one consistent row. Writing the exact column sets the binary64 column to
--   the nearest double; writing only the binary64 column sets the exact column to its
--   shortest decimal (`extra_float_digits = 1`), which is the decimal the JSON number
--   carried (`Decimal::from_wire`).
-- * Contract (a later release, once no binary that reads the binary64 columns serves):
--   NOT NULL and `>= 0` on `price_decimal`/`tax_rate_decimal`, then drop this trigger, its
--   function and the four binary64 columns.
ALTER TABLE public.products
    ADD COLUMN IF NOT EXISTS price_decimal numeric,
    ADD COLUMN IF NOT EXISTS tax_rate_decimal numeric,
    ADD COLUMN IF NOT EXISTS list_price_decimal numeric,
    ADD COLUMN IF NOT EXISTS regulation_price_decimal numeric;

CREATE OR REPLACE FUNCTION public.products_exact_prices() RETURNS trigger
    LANGUAGE plpgsql
    SET extra_float_digits TO '1'
    AS $$
BEGIN
 IF TG_OP = 'INSERT' THEN
  IF NEW.price_decimal IS NOT NULL THEN NEW.price := NEW.price_decimal::double precision;
  ELSE NEW.price_decimal := NEW.price::text::numeric; END IF;
  IF NEW.tax_rate_decimal IS NOT NULL THEN NEW.tax_rate := NEW.tax_rate_decimal::double precision;
  ELSE NEW.tax_rate_decimal := NEW.tax_rate::text::numeric; END IF;
  IF NEW.list_price_decimal IS NOT NULL THEN NEW.list_price := NEW.list_price_decimal::double precision;
  ELSE NEW.list_price_decimal := NEW.list_price::text::numeric; END IF;
  IF NEW.regulation_price_decimal IS NOT NULL THEN NEW.regulation_price := NEW.regulation_price_decimal::double precision;
  ELSE NEW.regulation_price_decimal := NEW.regulation_price::text::numeric; END IF;
  RETURN NEW;
 END IF;
 -- An update follows whichever representation the statement changed; the exact one wins.
 -- A required exact column left empty (a row the backfill has not reached) follows the binary64 one.
 IF NEW.price_decimal IS DISTINCT FROM OLD.price_decimal AND NEW.price_decimal IS NOT NULL THEN
  NEW.price := NEW.price_decimal::double precision;
 ELSIF NEW.price IS DISTINCT FROM OLD.price OR NEW.price_decimal IS NULL THEN
  NEW.price_decimal := NEW.price::text::numeric;
 END IF;
 IF NEW.tax_rate_decimal IS DISTINCT FROM OLD.tax_rate_decimal AND NEW.tax_rate_decimal IS NOT NULL THEN
  NEW.tax_rate := NEW.tax_rate_decimal::double precision;
 ELSIF NEW.tax_rate IS DISTINCT FROM OLD.tax_rate OR NEW.tax_rate_decimal IS NULL THEN
  NEW.tax_rate_decimal := NEW.tax_rate::text::numeric;
 END IF;
 IF NEW.list_price_decimal IS DISTINCT FROM OLD.list_price_decimal THEN
  NEW.list_price := NEW.list_price_decimal::double precision;
 ELSIF NEW.list_price IS DISTINCT FROM OLD.list_price OR (NEW.list_price_decimal IS NULL AND NEW.list_price IS NOT NULL) THEN
  NEW.list_price_decimal := NEW.list_price::text::numeric;
 END IF;
 IF NEW.regulation_price_decimal IS DISTINCT FROM OLD.regulation_price_decimal THEN
  NEW.regulation_price := NEW.regulation_price_decimal::double precision;
 ELSIF NEW.regulation_price IS DISTINCT FROM OLD.regulation_price OR (NEW.regulation_price_decimal IS NULL AND NEW.regulation_price IS NOT NULL) THEN
  NEW.regulation_price_decimal := NEW.regulation_price::text::numeric;
 END IF;
 RETURN NEW;
END $$;

DROP TRIGGER IF EXISTS exact_prices_sync ON public.products;

CREATE TRIGGER exact_prices_sync BEFORE INSERT OR UPDATE ON public.products FOR EACH ROW EXECUTE FUNCTION public.products_exact_prices();
-- veyndo:step
-- Backfill in batches of 500 rows, each committed on its own (a step of one statement, so the
-- block may COMMIT); rows the previous release writes meanwhile are converted by the trigger,
-- which also converts these rows: an exact column left empty follows its binary64 column.
-- Prices, revisions and content clocks stay unchanged. The product history records each
-- converted row as a migration change with this reason.
DO $$
DECLARE moved integer;
BEGIN
 PERFORM set_config('veyndo.source', 'migration', false), set_config('veyndo.reason', 'catalog/0002-exact-product-prices', false);
 LOOP
  UPDATE public.products p SET price_decimal = p.price_decimal
  WHERE (p.tenant, p.id) IN (
   SELECT tenant, id FROM public.products
   WHERE price_decimal IS NULL OR tax_rate_decimal IS NULL
      OR (list_price_decimal IS NULL AND list_price IS NOT NULL)
      OR (regulation_price_decimal IS NULL AND regulation_price IS NOT NULL)
   LIMIT 500 FOR UPDATE);
  GET DIAGNOSTICS moved = ROW_COUNT;
  EXIT WHEN moved = 0;
  COMMIT;
 END LOOP;
 PERFORM set_config('veyndo.source', '', false), set_config('veyndo.reason', '', false);
END $$;
