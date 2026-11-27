-- Units per product in the trailing window. Same placed/payment filter as the sales report; no customer projection.
SELECT coalesce(l->>'referencedId',l->>'id') AS "product_id!",
 coalesce(sum((l->>'quantity')::bigint),0)::bigint AS "units!",
 count(DISTINCT o.id)::bigint AS "orders!"
FROM orders o CROSS JOIN LATERAL jsonb_array_elements(coalesce(o.data#>'{cart,lineItems}','[]')) l
WHERE o.tenant=$1 AND o.created_at>=now()-make_interval(days=>$3::int)
 AND coalesce(o.data->>'state','placed') NOT IN ('cancelled','expired','payment_review')
 AND coalesce(o.data#>>'{payment,state}','') NOT IN ('failed','cancelled','expired')
 AND coalesce(l->>'type','product')='product'
 AND coalesce(l->>'referencedId',l->>'id')=ANY($2)
GROUP BY 1 ORDER BY 1
