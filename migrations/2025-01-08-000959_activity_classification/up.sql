CREATE TABLE app_classifications (
    application_name TEXT NOT NULL PRIMARY KEY, -- Foreign key to apps.name
    classification TEXT, -- Nullable column for classification
    FOREIGN KEY (application_name)
        REFERENCES apps (name)
        ON DELETE CASCADE -- Ensures referential integrity
);