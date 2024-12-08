-- Your SQL goes here
CREATE TABLE app (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT, -- Unique ID for each app
    app_name TEXT NOT NULL UNIQUE,         -- App name, must be unique
    app_path TEXT                          -- Optional app path
);
