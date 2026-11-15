-- Review 2026-11-15 #9: a failed index job was released at once and claimed
-- again 200 ms later, so a 2-second storage or Vault outage used its five
-- attempts in about a second. It now stays claimed: the 5-minute reclaim of
-- kura_index_claim is the wait between attempts.
CREATE OR REPLACE FUNCTION kura_index_failed(p_vault uuid, p_entry uuid) RETURNS void
    LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog, public
    AS $$
        UPDATE index_job SET attempts = attempts + 1, claimed_at = now(),
               failed_at = CASE WHEN attempts + 1 >= 5 THEN now() END
        WHERE vault_id = p_vault AND entry_id = p_entry AND claimed_at IS NOT NULL
    $$;
