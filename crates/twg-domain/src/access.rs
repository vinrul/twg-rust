use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum Role {
    Admin,
    AdminMonitor,
    AdminSatker,
    UserSatker,
    User,
    /// Dedicated account for a big-screen display. It may only open the
    /// big-screen page and watch the live simulation.
    BigScreen,
}

impl Role {
    pub const fn is_admin(self) -> bool {
        matches!(self, Self::Admin)
    }

    pub const fn is_admin_monitor(self) -> bool {
        matches!(self, Self::AdminMonitor)
    }

    pub const fn is_admin_like(self) -> bool {
        matches!(self, Self::Admin | Self::AdminMonitor | Self::AdminSatker)
    }

    pub const fn is_satker_role(self) -> bool {
        matches!(self, Self::AdminSatker | Self::UserSatker)
    }

    pub const fn sees_all_satkers(self) -> bool {
        matches!(self, Self::Admin | Self::AdminMonitor)
    }

    pub fn can_access_satker(
        self,
        actor_satker_id: Option<Uuid>,
        resource_satker_id: Option<Uuid>,
    ) -> bool {
        if self == Self::BigScreen {
            return false;
        }
        if self.sees_all_satkers() {
            return true;
        }

        if self == Self::User {
            return resource_satker_id.is_none();
        }

        match (actor_satker_id, resource_satker_id) {
            (Some(actor), Some(resource)) => actor == resource,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSubject {
    pub user_id: Uuid,
    pub role: Role,
    pub satker_id: Option<Uuid>,
}

impl SessionSubject {
    pub fn can_access_satker(&self, resource_satker_id: Option<Uuid>) -> bool {
        self.role
            .can_access_satker(self.satker_id, resource_satker_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataScope {
    All,
    Satker(Uuid),
    Unscoped,
}

impl DataScope {
    pub fn for_subject(subject: &SessionSubject) -> Self {
        if subject.role.sees_all_satkers() {
            return Self::All;
        }
        if subject.role == Role::User {
            return Self::Unscoped;
        }
        match subject.satker_id {
            Some(satker_id) => Self::Satker(satker_id),
            None => Self::Unscoped,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DataScope, Role, SessionSubject};
    use uuid::Uuid;

    fn id(value: u128) -> Uuid {
        Uuid::from_u128(value)
    }

    #[test]
    fn admin_like_roles_see_all_satkers() {
        for role in [Role::Admin, Role::AdminMonitor] {
            assert!(role.can_access_satker(Some(id(1)), Some(id(2))));
            assert!(role.can_access_satker(None, Some(id(2))));
        }
    }

    #[test]
    fn satker_roles_are_scoped_to_the_same_satker() {
        let actor = Some(id(1));
        assert!(Role::AdminSatker.can_access_satker(actor, Some(id(1))));
        assert!(Role::UserSatker.can_access_satker(actor, Some(id(1))));
        assert!(!Role::AdminSatker.can_access_satker(actor, Some(id(2))));
        assert!(!Role::UserSatker.can_access_satker(actor, None));
    }

    #[test]
    fn user_only_sees_unscoped_resources() {
        assert!(Role::User.can_access_satker(None, None));
        assert!(!Role::User.can_access_satker(Some(id(1)), Some(id(1))));
    }

    #[test]
    fn session_subject_uses_role_scope() {
        let subject = SessionSubject {
            user_id: id(10),
            role: Role::UserSatker,
            satker_id: Some(id(1)),
        };

        assert!(subject.can_access_satker(Some(id(1))));
        assert!(!subject.can_access_satker(Some(id(2))));
    }

    #[test]
    fn role_serializes_as_snake_case() {
        assert_eq!(
            serde_json::to_string(&Role::AdminSatker).unwrap(),
            "\"admin_satker\""
        );
        assert_eq!(
            serde_json::from_str::<Role>("\"user_satker\"").unwrap(),
            Role::UserSatker
        );
        assert_eq!(
            serde_json::from_str::<Role>("\"big_screen\"").unwrap(),
            Role::BigScreen
        );
        assert_eq!(Role::BigScreen.to_string(), "big_screen");
    }

    #[test]
    fn big_screen_role_cannot_access_scenarios() {
        assert!(!Role::BigScreen.can_access_satker(None, None));
        assert!(!Role::BigScreen.can_access_satker(Some(id(1)), Some(id(1))));
        assert!(!Role::BigScreen.is_admin_like());
        assert!(!Role::BigScreen.is_satker_role());
    }

    #[test]
    fn data_scope_is_derived_from_role() {
        let all = SessionSubject {
            user_id: id(1),
            role: Role::Admin,
            satker_id: None,
        };
        let satker = SessionSubject {
            user_id: id(2),
            role: Role::AdminSatker,
            satker_id: Some(id(9)),
        };
        let unscoped = SessionSubject {
            user_id: id(3),
            role: Role::User,
            satker_id: Some(id(9)),
        };

        assert_eq!(DataScope::for_subject(&all), DataScope::All);
        assert_eq!(DataScope::for_subject(&satker), DataScope::Satker(id(9)));
        assert_eq!(DataScope::for_subject(&unscoped), DataScope::Unscoped);
    }
}
