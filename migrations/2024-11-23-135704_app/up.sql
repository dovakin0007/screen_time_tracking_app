-- Your SQL goes here
CREATE TABLE app (
    id uuid PRIMARY KEY,
    app_name text NOT NULL UNIQUE,
    app_path text
);

