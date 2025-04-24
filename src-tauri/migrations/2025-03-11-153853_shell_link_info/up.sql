CREATE TABLE shell_link_info (
    link TEXT UNIQUE,
    target_path TEXT NOT NULL,
    arguments TEXT,
    icon_base64_image TEXT,
    working_directory TEXT,
    description TEXT
);