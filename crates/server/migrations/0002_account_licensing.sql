-- Licensing lives on the account: each organization has a plan and the date
-- it ends. While it runs, desks get a signed lease from the server.
-- New accounts are created on a trial by the Worker (server::TRIAL_DAYS);
-- paid plans are set with scripts/license-account.sh.
ALTER TABLE organizations
    ADD COLUMN license_plan TEXT NOT NULL DEFAULT 'trial'
        CHECK (license_plan IN ('trial', 'semester', '1-year', '3-year', '5-year')),
    ADD COLUMN license_expires_on DATE NOT NULL DEFAULT (current_date + 30);
-- The Worker always supplies the trial end, so the trial length has one source.
ALTER TABLE organizations ALTER COLUMN license_expires_on DROP DEFAULT;
-- `licensed` was only ever set by hand, and no customer existed when this
-- shipped (2026-11-15), so it is dropped now rather than in a follow-up.
ALTER TABLE organizations DROP COLUMN licensed;
