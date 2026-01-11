-- Users
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY NOT NULL,
    email TEXT UNIQUE NOT NULL,
    verification_token TEXT NOT NULL,
    is_verified BOOLEAN NOT NULL DEFAULT 0,
    is_subscribed BOOLEAN NOT NULL DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TRIGGER update_user_timestamp 
AFTER UPDATE ON users
BEGIN
   UPDATE users SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

-- Tide predictions
CREATE TABLE IF NOT EXISTS tides (
    prediction_time DATETIME PRIMARY KEY NOT NULL,
    height_ft REAL NOT NULL,
    tide_type TEXT CHECK( tide_type IN ('High', 'Low') ),
    last_updated DATETIME DEFAULT CURRENT_TIMESTAMP
);


-- Mailing list view
CREATE VIEW IF NOT EXISTS mailing_list AS
    SELECT id, email FROM users
    WHERE is_verified = 1 AND is_subscribed = 1;
