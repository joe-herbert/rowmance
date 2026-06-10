-- Temporary plain-text password column for Phase 1.
-- Replaced by OS keychain storage (keyring crate) in Phase 3.
ALTER TABLE connection_profiles ADD COLUMN password TEXT;
