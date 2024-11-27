-- Your SQL goes here
CREATE TABLE app_usage (
    id uuid PRIMARY KEY,
    app_name text NOT NULL REFERENCES app(app_name),
    screen_title_name text UNIQUE,
    duration_in_seconds integer NOT NULL CHECK (duration_in_seconds >= 0),
    date date NOT NULL DEFAULT CURRENT_TIMESTAMP,
    time_stamp timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at timestamp DEFAULT CURRENT_TIMESTAMP,
    updated_at timestamp DEFAULT CURRENT_TIMESTAMP
);
