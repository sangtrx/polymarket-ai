CREATE TABLE IF NOT EXISTS roles (
    role_id SMALLINT PRIMARY KEY,
    role_key TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (role_key IN ('read_only_analytics', 'operational_control', 'administrative_actions'))
);

CREATE TABLE IF NOT EXISTS role_permissions (
    role_id SMALLINT NOT NULL,
    permission_key TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (role_id, permission_key),
    FOREIGN KEY (role_id) REFERENCES roles(role_id) ON DELETE CASCADE,
    CHECK (permission_key IN ('read_analytics', 'execute_control_action', 'manage_role_assignments'))
);

CREATE TABLE IF NOT EXISTS user_roles (
    actor_id TEXT NOT NULL,
    role_id SMALLINT NOT NULL,
    assigned_by TEXT NOT NULL,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (actor_id, role_id),
    FOREIGN KEY (role_id) REFERENCES roles(role_id) ON DELETE RESTRICT
);

INSERT INTO roles (role_id, role_key, description)
VALUES
    (10, 'read_only_analytics', 'Read-only analytics access'),
    (20, 'operational_control', 'Operational control actions'),
    (30, 'administrative_actions', 'Administrative role-management actions')
ON CONFLICT (role_key) DO UPDATE
SET description = EXCLUDED.description,
    updated_at = NOW();

INSERT INTO role_permissions (role_id, permission_key)
SELECT r.role_id, p.permission_key
FROM roles r
JOIN (
    VALUES
        ('read_only_analytics', 'read_analytics'),
        ('operational_control', 'read_analytics'),
        ('operational_control', 'execute_control_action'),
        ('administrative_actions', 'read_analytics'),
        ('administrative_actions', 'execute_control_action'),
        ('administrative_actions', 'manage_role_assignments')
) AS p(role_key, permission_key)
ON p.role_key = r.role_key
ON CONFLICT (role_id, permission_key) DO NOTHING;
