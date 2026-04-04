use domain::governance::{GovernancePermission, GovernanceRole};
use sqlx::{PgExecutor, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RbacPersistenceError {
    pub code: &'static str,
    pub message: String,
}

impl RbacPersistenceError {
    fn query_failure(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "rbac_query_failed",
            message: format!("{operation} failed: {error}"),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "rbac_row_decode_failed",
            message: format!("unable to decode `{column}`: {error}"),
        }
    }

    fn role_mapping_not_found(role_key: &str) -> Self {
        Self {
            code: "rbac_role_mapping_not_found",
            message: format!("role mapping for `{role_key}` is not configured"),
        }
    }

    fn unknown_role_key(role_key: &str) -> Self {
        Self {
            code: "rbac_unknown_role_key",
            message: format!("database returned unknown role key `{role_key}`"),
        }
    }

    fn unknown_permission_key(permission_key: &str) -> Self {
        Self {
            code: "rbac_unknown_permission_key",
            message: format!("database returned unknown permission key `{permission_key}`"),
        }
    }
}

impl Display for RbacPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for RbacPersistenceError {}

pub async fn fetch_role_permissions<'e, E>(
    executor: E,
    role: GovernanceRole,
) -> Result<Vec<GovernancePermission>, RbacPersistenceError>
where
    E: PgExecutor<'e>,
{
    let rows = sqlx::query(
        r#"
        SELECT rp.permission_key
        FROM role_permissions rp
        INNER JOIN roles r ON r.role_id = rp.role_id
        WHERE r.role_key = $1
        ORDER BY rp.permission_key
        "#,
    )
    .bind(role.as_str())
    .fetch_all(executor)
    .await
    .map_err(|error| RbacPersistenceError::query_failure("fetch_role_permissions", error))?;

    let mut permissions = Vec::with_capacity(rows.len());
    for row in rows {
        let permission_key: String = row
            .try_get("permission_key")
            .map_err(|error| RbacPersistenceError::row_decode_failure("permission_key", error))?;
        let permission = GovernancePermission::parse(&permission_key)
            .map_err(|_| RbacPersistenceError::unknown_permission_key(&permission_key))?;
        permissions.push(permission);
    }

    Ok(permissions)
}

pub async fn fetch_user_roles<'e, E>(
    executor: E,
    actor_id: &str,
) -> Result<Vec<GovernanceRole>, RbacPersistenceError>
where
    E: PgExecutor<'e>,
{
    let rows = sqlx::query(
        r#"
        SELECT r.role_key
        FROM user_roles ur
        INNER JOIN roles r ON r.role_id = ur.role_id
        WHERE ur.actor_id = $1
        ORDER BY r.role_key
        "#,
    )
    .bind(actor_id)
    .fetch_all(executor)
    .await
    .map_err(|error| RbacPersistenceError::query_failure("fetch_user_roles", error))?;

    let mut roles = Vec::with_capacity(rows.len());
    for row in rows {
        let role_key: String = row
            .try_get("role_key")
            .map_err(|error| RbacPersistenceError::row_decode_failure("role_key", error))?;
        let role = GovernanceRole::parse(&role_key)
            .map_err(|_| RbacPersistenceError::unknown_role_key(&role_key))?;
        roles.push(role);
    }

    Ok(roles)
}

pub async fn assign_user_role<'e, E>(
    executor: E,
    actor_id: &str,
    role: GovernanceRole,
    assigned_by: &str,
) -> Result<(), RbacPersistenceError>
where
    E: PgExecutor<'e>,
{
    let query_result = sqlx::query(
        r#"
        INSERT INTO user_roles (actor_id, role_id, assigned_by)
        SELECT $1, r.role_id, $2
        FROM roles r
        WHERE r.role_key = $3
        "#,
    )
    .bind(actor_id)
    .bind(assigned_by)
    .bind(role.as_str())
    .execute(executor)
    .await
    .map_err(|error| RbacPersistenceError::query_failure("assign_user_role", error))?;

    if query_result.rows_affected() != 1 {
        return Err(RbacPersistenceError::role_mapping_not_found(role.as_str()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    const RBAC_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260405025656_rbac_roles_permissions.sql");

    #[test]
    fn migration_contains_expected_rbac_tables() {
        assert!(RBAC_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS roles"));
        assert!(RBAC_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS role_permissions"));
        assert!(RBAC_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS user_roles"));
    }

    #[test]
    fn migration_enforces_role_and_permission_boundaries() {
        assert!(RBAC_MIGRATION_SQL.contains("CHECK (role_key IN"));
        assert!(RBAC_MIGRATION_SQL.contains("CHECK (permission_key IN"));
    }

    #[test]
    fn migration_enforces_fk_and_duplicate_assignment_constraints() {
        assert!(RBAC_MIGRATION_SQL.contains("FOREIGN KEY (role_id) REFERENCES roles(role_id)"));
        assert!(RBAC_MIGRATION_SQL.contains("PRIMARY KEY (role_id, permission_key)"));
        assert!(RBAC_MIGRATION_SQL.contains("PRIMARY KEY (actor_id, role_id)"));
    }
}
