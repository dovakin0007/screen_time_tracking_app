-- Your SQL goes here
CREATE TABLE sessions (
    id TEXT PRIMARY KEY, -- Unique identifier for each session
    date DATE NOT NULL -- Date of the session
);

CREATE INDEX idx_sessions_date ON sessions (date);