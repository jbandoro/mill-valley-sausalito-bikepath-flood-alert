-- Users table
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY NOT NULL, -- UUID
    email TEXT UNIQUE NOT NULL,
    verification_token TEXT NOT NULL,
    is_verified BOOLEAN NOT NULL DEFAULT 0,
    is_subscribed BOOLEAN NOT NULL DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Cache for tide predictions
CREATE TABLE IF NOT EXISTS tides (
    prediction_time DATETIME PRIMARY KEY NOT NULL,
    height_ft REAL NOT NULL,
    tide_type TEXT CHECK( tide_type IN ('High', 'Low') ),
    last_updated DATETIME DEFAULT CURRENT_TIMESTAMP
);
