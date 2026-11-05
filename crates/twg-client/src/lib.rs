use serde::{Deserialize, Serialize};
use twg_domain::Role;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Screen {
    #[default]
    Boot,
    Login,
    Workspace,
    MapEditor,
    Preview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientSession {
    pub user_id: Uuid,
    pub username: String,
    pub role: Role,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub satker_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClientState {
    pub screen: Screen,
    pub session: Option<ClientSession>,
}

impl ClientState {
    pub fn login(&mut self, session: ClientSession) {
        self.session = Some(session);
        self.screen = Screen::Workspace;
    }

    pub fn logout(&mut self) {
        self.session = None;
        self.screen = Screen::Login;
    }
}

#[cfg(test)]
mod tests {
    use super::{ClientSession, ClientState, Screen};
    use twg_domain::Role;
    use uuid::Uuid;

    #[test]
    fn login_moves_to_workspace() {
        let mut state = ClientState::default();
        state.login(ClientSession {
            user_id: Uuid::nil(),
            username: "baselineadmin".to_owned(),
            role: Role::Admin,
            satker_id: None,
        });

        assert_eq!(state.screen, Screen::Workspace);
        assert!(state.session.is_some());
    }

    #[test]
    fn logout_clears_session() {
        let mut state = ClientState {
            screen: Screen::MapEditor,
            session: Some(ClientSession {
                user_id: Uuid::nil(),
                username: "baselineadmin".to_owned(),
                role: Role::Admin,
                satker_id: None,
            }),
        };
        state.logout();
        assert_eq!(state.screen, Screen::Login);
        assert!(state.session.is_none());
    }
}
