CREATE TABLE IF NOT EXISTS nodes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL CHECK (status IN ('Running', 'Stopped')),
    image_path TEXT NOT NULL,
    overlay_path TEXT,
    vnc_port INTEGER,
    guacamole_connection_id TEXT
);
