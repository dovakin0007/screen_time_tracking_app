CREATE TABLE activity_classifications (
    application_name TEXT NOT NULL, -- Foreign key to app_usages.application_name
    current_screen_title TEXT NOT NULL, -- Foreign key to app_usages.current_screen_title
    classification TEXT, -- Nullable column for classification
    FOREIGN KEY (application_name)
        REFERENCES apps (name)
        ON DELETE CASCADE, -- Optional: Ensures referential integrity
    UNIQUE (current_screen_title) -- Composite unique constraint
);