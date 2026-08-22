ALTER TABLE owners RENAME COLUMN email TO username;

ALTER TABLE login_attempts RENAME COLUMN email_hash TO username_hash;
