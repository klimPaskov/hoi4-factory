//! Domain types shared by the repositories, the service and the HTTP layer.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::ids::{AccountId, ProjectId};

/// Project role. Stored as text in `project_members.role`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Owner,
    Maintainer,
    Contributor,
    Viewer,
}

/// An action on a project that a role may or may not permit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    /// Read source, previews and evidence.
    ReadProject,
    /// Create tasks and proposals on owned branches.
    CreateTask,
    /// Approve and integrate proposals.
    ApproveProposal,
    /// Publish within granted scope.
    Publish,
    RenameProject,
    ChangeVisibility,
    MarkForDeletion,
    ManageMembers,
}

impl Role {
    pub const ALL: [Role; 4] = [
        Role::Owner,
        Role::Maintainer,
        Role::Contributor,
        Role::Viewer,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Role::Owner => "owner",
            Role::Maintainer => "maintainer",
            Role::Contributor => "contributor",
            Role::Viewer => "viewer",
        }
    }

    pub fn parse(value: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|r| r.as_str() == value)
    }

    /// The permission matrix from SPEC-005 (roles) and SPEC-011 (grants never exceed roles).
    pub fn allows(self, permission: Permission) -> bool {
        use Permission::*;
        match self {
            Role::Owner => true,
            Role::Maintainer => matches!(
                permission,
                ReadProject | CreateTask | ApproveProposal | Publish | RenameProject
            ),
            Role::Contributor => matches!(permission, ReadProject | CreateTask),
            Role::Viewer => matches!(permission, ReadProject),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Private,
    Public,
}

impl Visibility {
    pub fn as_str(self) -> &'static str {
        match self {
            Visibility::Private => "private",
            Visibility::Public => "public",
        }
    }

    pub fn parse(value: &str) -> Option<Visibility> {
        match value {
            "private" => Some(Visibility::Private),
            "public" => Some(Visibility::Public),
            _ => None,
        }
    }
}

/// An account as shown to its own holder.
#[derive(Debug, Clone, Serialize)]
pub struct Account {
    pub id: AccountId,
    pub email: String,
    /// False until the verification link is used, or immediately true after GitHub sign-in.
    /// Agent tasks are gated on this flag by later stages (see `Principal::require_verified`).
    pub email_verified: bool,
    pub display_name: String,
    pub github_linked: bool,
    pub has_password: bool,
    pub created_at: DateTime<Utc>,
}

/// A project as seen by one account.
#[derive(Debug, Clone, Serialize)]
pub struct Project {
    pub id: ProjectId,
    /// Needed server-side to resolve workspace paths. Not part of the browser view.
    #[serde(skip)]
    pub owner_account_id: AccountId,
    pub name: String,
    pub visibility: Visibility,
    pub created_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub delete_after: Option<DateTime<Utc>>,
    /// The caller's role, or `None` for read-only access to a public project.
    pub role: Option<Role>,
}

impl Project {
    /// Soft deleted: hidden from listings until the grace period ends.
    pub fn pending_deletion(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// The role check used by every mutating repository method.
    pub(crate) fn require(&self, permission: Permission) -> Result<(), ApiError> {
        match self.role {
            Some(role) if role.allows(permission) => Ok(()),
            _ => Err(
                ApiError::forbidden("Your role on this project does not allow that action.")
                    .with_id("project_id", self.id),
            ),
        }
    }
}

/// One row of a project's member list.
#[derive(Debug, Clone, Serialize)]
pub struct Member {
    pub account_id: AccountId,
    pub role: Role,
    pub added_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_matrix() {
        use Permission::*;
        let all = [
            ReadProject,
            CreateTask,
            ApproveProposal,
            Publish,
            RenameProject,
            ChangeVisibility,
            MarkForDeletion,
            ManageMembers,
        ];
        let cases: [(Role, &[Permission]); 4] = [
            (Role::Owner, &all),
            (
                Role::Maintainer,
                &[
                    ReadProject,
                    CreateTask,
                    ApproveProposal,
                    Publish,
                    RenameProject,
                ],
            ),
            (Role::Contributor, &[ReadProject, CreateTask]),
            (Role::Viewer, &[ReadProject]),
        ];
        for (role, allowed) in cases {
            for p in all {
                assert_eq!(role.allows(p), allowed.contains(&p), "{role:?} {p:?}");
            }
        }
    }

    #[test]
    fn role_text_round_trips() {
        for r in Role::ALL {
            assert_eq!(Role::parse(r.as_str()), Some(r));
        }
        assert_eq!(Role::parse("admin"), None);
    }
}
