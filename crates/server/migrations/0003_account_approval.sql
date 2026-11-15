-- Every new account waits for the owner's approval before it can be used.
-- NULL = pending. Set with `scripts/license-account.sh <email> approve`.
-- The trial clock (license_expires_on) still runs from sign-up.
ALTER TABLE organizations ADD COLUMN approved_at TIMESTAMPTZ;
