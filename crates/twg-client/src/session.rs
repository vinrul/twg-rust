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
    /// Public big-screen display for the Simulasi play session. No login is
    /// required; the camera and play state follow the active admin session.
    BigScreen,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NavItem {
    #[default]
    Dashboard,
    Operations,
    Scenarios,
    Assets,
    AssetGroups,
    Games,
    Users,
    Satkers,
    Online,
    Snapshots,
    Preview,
    BigScreen,
    Profile,
}

impl NavItem {
    pub const ALL: [Self; 13] = [
        Self::Dashboard,
        Self::Operations,
        Self::Scenarios,
        Self::Assets,
        Self::AssetGroups,
        Self::Games,
        Self::Users,
        Self::Satkers,
        Self::Online,
        Self::Snapshots,
        Self::Preview,
        Self::BigScreen,
        Self::Profile,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Operations => "Operasi",
            Self::Scenarios => "Skenario",
            Self::Assets => "Aset",
            Self::AssetGroups => "Kategori Aset",
            Self::Games => "Simulasi",
            Self::Users => "Pengguna",
            Self::Satkers => "Satker",
            Self::Online => "Online",
            Self::Snapshots => "Snapshot",
            Self::Preview => "Peta",
            Self::BigScreen => "Layar Besar",
            Self::Profile => "Profil",
        }
    }

    pub const fn icon(self) -> &'static str {
        match self {
            Self::Dashboard => "#",
            Self::Operations => "OP",
            Self::Scenarios => "SK",
            Self::Assets => "AS",
            Self::AssetGroups => "KA",
            Self::Games => "SM",
            Self::Users => "US",
            Self::Satkers => "ST",
            Self::Online => "@",
            Self::Snapshots => "SN",
            Self::Preview => "MP",
            Self::BigScreen => "LB",
            Self::Profile => "PR",
        }
    }

    pub const fn screen(self) -> Screen {
        match self {
            Self::Preview => Screen::MapEditor,
            Self::BigScreen => Screen::BigScreen,
            _ => Screen::Workspace,
        }
    }

    pub fn allowed_for(self, role: Role) -> bool {
        if role == Role::BigScreen {
            return self == Self::BigScreen;
        }
        match self {
            Self::Users => role.is_admin_like(),
            Self::Satkers => true,
            _ => true,
        }
    }

    pub fn visible_items(role: Role) -> impl Iterator<Item = Self> {
        Self::ALL
            .into_iter()
            .filter(move |item| item.allowed_for(role))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientSession {
    pub user_id: Uuid,
    pub name: String,
    pub username: String,
    pub role: Role,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub satker_id: Option<Uuid>,
    pub profile_img: String,
}

impl ClientSession {
    pub fn can_write_operations(&self) -> bool {
        self.role.is_admin_like()
    }

    pub fn can_write_users(&self) -> bool {
        self.role.is_admin_like()
    }

    pub fn can_write_satkers(&self) -> bool {
        self.role == Role::Admin
    }

    pub fn can_see_all_satkers(&self) -> bool {
        self.role.sees_all_satkers()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClientState {
    pub screen: Screen,
    pub session: Option<ClientSession>,
    pub nav: NavItem,
}

impl ClientState {
    pub fn login(&mut self, session: ClientSession) {
        self.session = Some(session);
        self.screen = Screen::Workspace;
        self.nav = NavItem::Dashboard;
    }

    pub fn logout(&mut self) {
        self.session = None;
        self.screen = Screen::Login;
        self.nav = NavItem::Dashboard;
    }

    pub fn navigate(&mut self, item: NavItem) {
        let Some(session) = &self.session else {
            self.screen = Screen::Login;
            return;
        };
        if !item.allowed_for(session.role) {
            return;
        }
        self.nav = item;
        self.screen = item.screen();
    }
}

#[cfg(test)]
mod tests {
    use super::{ClientSession, ClientState, NavItem, Screen};
    use twg_domain::Role;
    use uuid::Uuid;

    fn session(role: Role) -> ClientSession {
        ClientSession {
            user_id: Uuid::nil(),
            name: "User".to_owned(),
            username: "user".to_owned(),
            role,
            satker_id: None,
            profile_img: String::new(),
        }
    }

    #[test]
    fn login_moves_to_workspace() {
        let mut state = ClientState::default();
        state.login(session(Role::Admin));
        assert_eq!(state.screen, Screen::Workspace);
        assert_eq!(state.nav, NavItem::Dashboard);
    }

    #[test]
    fn logout_clears_session() {
        let mut state = ClientState::default();
        state.login(session(Role::Admin));
        state.logout();
        assert_eq!(state.screen, Screen::Login);
        assert!(state.session.is_none());
    }

    #[test]
    fn preview_switches_screen() {
        let mut state = ClientState::default();
        state.login(session(Role::User));
        state.navigate(NavItem::Preview);
        assert_eq!(state.screen, Screen::MapEditor);
    }

    #[test]
    fn every_role_can_open_the_public_big_screen() {
        for role in [
            Role::Admin,
            Role::AdminMonitor,
            Role::AdminSatker,
            Role::User,
            Role::BigScreen,
        ] {
            assert!(NavItem::BigScreen.allowed_for(role));
            let mut state = ClientState::default();
            state.login(session(role));
            state.navigate(NavItem::BigScreen);
            assert_eq!(state.screen, Screen::BigScreen);
        }
    }

    #[test]
    fn big_screen_role_only_sees_the_big_screen_nav() {
        let items = NavItem::visible_items(Role::BigScreen).collect::<Vec<_>>();
        assert_eq!(items, vec![NavItem::BigScreen]);
    }

    #[test]
    fn regular_user_cannot_open_user_management() {
        let mut state = ClientState::default();
        state.login(session(Role::User));
        state.navigate(NavItem::Users);
        assert_eq!(state.nav, NavItem::Dashboard);
        assert_eq!(state.screen, Screen::Workspace);
    }

    #[test]
    fn admin_satker_can_open_user_management_but_not_write_satkers() {
        let session = session(Role::AdminSatker);
        assert!(NavItem::Users.allowed_for(session.role));
        assert!(session.can_write_users());
        assert!(!session.can_write_satkers());
    }

    #[test]
    fn nav_visibility_follows_role() {
        let user_items = NavItem::visible_items(Role::User).collect::<Vec<_>>();
        assert!(!user_items.contains(&NavItem::Users));
        // Everyone may browse the asset catalog; writes are gated server-side.
        assert!(user_items.contains(&NavItem::Assets));
        let admin_items = NavItem::visible_items(Role::Admin).collect::<Vec<_>>();
        assert!(admin_items.contains(&NavItem::Users));
    }

    #[test]
    fn assets_nav_opens_the_workspace_screen() {
        let mut state = ClientState::default();
        state.login(session(Role::Admin));
        state.navigate(NavItem::Assets);
        assert_eq!(state.nav, NavItem::Assets);
        assert_eq!(state.screen, Screen::Workspace);
    }
}
