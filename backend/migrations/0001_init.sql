-- Images table for the layered image hierarchy
-- Images can be base images (parent_id = NULL) or overlays pointing to a parent
CREATE TABLE IF NOT EXISTS images (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL UNIQUE,
    path TEXT NOT NULL UNIQUE,
    parent_id UUID REFERENCES images(id) ON DELETE RESTRICT,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_images_parent_id ON images(parent_id);

-- Nodes table for VM instances
-- Each node is based on an image and has its own runtime overlay
CREATE TABLE IF NOT EXISTS nodes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL CHECK (status IN ('Running', 'Stopped')) DEFAULT 'Stopped',
    image_id UUID NOT NULL REFERENCES images(id) ON DELETE RESTRICT,
    instance_overlay_path TEXT NOT NULL UNIQUE,
    vnc_port INTEGER,
    guacamole_connection_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_nodes_image_id ON nodes(image_id);
