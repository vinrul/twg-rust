pub mod auth {
    pub const LOGIN: &str = "/auth/login";
    pub const REFRESH: &str = "/auth/refresh";
    pub const LOGOUT: &str = "/auth/logout";
    pub const IDENTIFY: &str = "/private/identify";
}

pub mod health {
    pub const LIVE: &str = "/healthz";
    pub const READY: &str = "/readyz";
}

pub mod categories {
    use std::fmt::Write;

    pub const CREATE: &str = "/admin/categories";
    pub const ALL: &str = "/private/categories/all";

    pub fn paginate(page_size: usize, page_number: usize, search: &str) -> String {
        let mut path = String::with_capacity(64 + search.len());
        let _ = write!(
            path,
            "/admin/categories?pageSize={page_size}&pageNumber={page_number}&search={search}"
        );
        path
    }

    pub fn by_id(id: &str) -> String {
        format!("/admin/categories/{id}")
    }
}

pub mod operations {
    pub const CREATE: &str = "/admin/operasis";
    pub const HOME: &str = "/public/list/operasis";
    pub const HOME_PRIVATE: &str = "/private/home/operasis";
    pub const ALL_PUBLIC: &str = "/public/operasis/all";

    pub fn paginate(page_size: usize, page_number: usize, search: &str) -> String {
        format!("/private/operasis?pageSize={page_size}&pageNumber={page_number}&search={search}")
    }

    pub fn by_id(id: &str) -> String {
        format!("/private/operasis/{id}")
    }

    pub fn update(id: &str) -> String {
        format!("/admin/operasis/{id}")
    }

    pub fn delete(id: &str) -> String {
        format!("/admin/operasis/{id}")
    }

    pub fn preview(id: &str) -> String {
        format!("/public/get/home/operasis/{id}")
    }
}

pub mod scenarios {
    pub const CREATE: &str = "/admin/skenarios";
    pub const COPY: &str = "/admin/skenarios/copy";

    pub fn by_operation(operation_id: &str) -> String {
        format!("/public/skenarios/{operation_id}")
    }

    pub fn by_id(id: &str) -> String {
        format!("/public/skenarios/get/{id}")
    }

    pub fn update(id: &str) -> String {
        format!("/admin/skenarios/{id}")
    }

    pub fn update_name(id: &str) -> String {
        format!("/admin/skenarios/name/{id}")
    }

    pub fn delete(id: &str) -> String {
        format!("/admin/skenarios/{id}")
    }
}

pub mod markers {
    pub const CREATE: &str = "/private/markers";

    pub fn list(scenario_id: &str) -> String {
        format!("/public/markers/list/{scenario_id}")
    }

    pub fn delete(id: &str) -> String {
        format!("/private/markers/{id}")
    }

    pub fn geometry(id: &str) -> String {
        format!("/private/markers/geom/{id}")
    }

    pub fn name(id: &str) -> String {
        format!("/private/markers/name/{id}")
    }

    pub fn scale(id: &str) -> String {
        format!("/private/markers/scale/{id}")
    }

    pub fn altitude(id: &str) -> String {
        format!("/private/markers/altitude/{id}")
    }

    pub fn rotation(id: &str) -> String {
        format!("/private/markers/rotasi/{id}")
    }

    pub fn duplicate(id: &str) -> String {
        format!("/private/markers/duplicate/{id}")
    }
}

pub mod buildings {
    pub const CREATE: &str = "/private/buildings";

    pub fn list(scenario_id: &str) -> String {
        format!("/public/buildings/list/{scenario_id}")
    }

    pub fn update(id: &str) -> String {
        format!("/private/buildings/{id}")
    }

    pub fn geometry(id: &str) -> String {
        format!("/private/buildings/geom/{id}")
    }

    pub fn delete(id: &str) -> String {
        format!("/private/buildings/{id}")
    }
}

pub mod lines {
    pub const CREATE: &str = "/private/lines";

    pub fn list(scenario_id: &str) -> String {
        format!("/public/lines/list/{scenario_id}")
    }

    pub fn update(id: &str) -> String {
        format!("/private/lines/{id}")
    }

    pub fn geometry(id: &str) -> String {
        format!("/private/lines/geom/{id}")
    }

    pub fn delete(id: &str) -> String {
        format!("/private/lines/{id}")
    }
}

pub mod text_markers {
    pub const CREATE: &str = "/private/text-markers";

    pub fn list(scenario_id: &str) -> String {
        format!("/public/text-markers/list/{scenario_id}")
    }

    pub fn update(id: &str) -> String {
        format!("/private/text-markers/{id}")
    }

    pub fn geometry(id: &str) -> String {
        format!("/private/text-markers/geom/{id}")
    }

    pub fn delete(id: &str) -> String {
        format!("/private/text-markers/{id}")
    }
}

pub mod animations {
    pub const CREATE_ROUTE: &str = "/private/anims/route";
    pub const CREATE_ROUTE_V2: &str = "/private/anims/route/v2";
    pub const CREATE_SPAWN: &str = "/private/anims/spawn";
    pub const CREATE_SPAWN_PICKUP: &str = "/private/anims/spawn_pickup";
    pub const CREATE_REMOVE: &str = "/private/anims/remove";
    pub const CREATE_COMBINE: &str = "/private/anims/combine";
    pub const CREATE_COMBINE_V2: &str = "/private/anims/combine/v2";
    pub const CREATE_SHOOT: &str = "/private/anims/shoot";
    pub const CREATE_SHOOT_V2: &str = "/private/anims/shoot/v2";
    pub const CREATE_PARTICLE: &str = "/private/anims/spawn_particle";
    pub const CREATE_REMOVE_PARTICLE: &str = "/private/anims/remove_particle";
    pub const ORDER: &str = "/admin/anims/urut";

    pub fn list(scenario_id: &str) -> String {
        format!("/public/anims/list/{scenario_id}")
    }

    pub fn route_data(id: &str) -> String {
        format!("/private/anims/route/data/{id}")
    }

    pub fn route_data_v2(id: &str) -> String {
        format!("/private/anims/route/data/v2/{id}")
    }

    pub fn spawn_data(id: &str) -> String {
        format!("/private/anims/spawn/data/{id}")
    }

    pub fn particle_data(id: &str) -> String {
        format!("/private/anims/particle/data/{id}")
    }

    pub fn remove_particle_data(id: &str) -> String {
        format!("/private/anims/remove_particle/data/{id}")
    }

    pub fn remove_data(id: &str) -> String {
        format!("/private/anims/remove/data/{id}")
    }

    pub fn combine_data(id: &str) -> String {
        format!("/private/anims/combine/data/{id}")
    }

    pub fn shoot_data(id: &str) -> String {
        format!("/private/anims/shoot/data/{id}")
    }

    pub fn combine_delete(id: &str) -> String {
        format!("/private/anims/combine/delete/{id}")
    }

    pub fn route_geometry(id: &str) -> String {
        format!("/private/anims/route/geom/{id}")
    }

    pub fn route_geometry_v2(id: &str) -> String {
        format!("/private/anims/route/geom/v2/{id}")
    }

    pub fn delete(id: &str) -> String {
        format!("/private/anims/{id}")
    }

    pub fn delete_spawn(id: &str) -> String {
        format!("/private/anims/spawn/{id}")
    }

    pub fn delete_combine(id: &str) -> String {
        format!("/private/anims/combine/{id}")
    }

    pub fn delete_shoot(id: &str) -> String {
        format!("/private/anims/shoot/{id}")
    }

    pub fn delete_particle(id: &str) -> String {
        format!("/private/anims/particle/{id}")
    }
}

pub mod particles {
    pub fn list(scenario_id: &str) -> String {
        format!("/public/particles/list/{scenario_id}")
    }
}

pub mod alurs {
    pub const CREATE: &str = "/admin/alurs";

    pub fn list(scenario_id: &str) -> String {
        format!("/public/alurs/list/{scenario_id}")
    }

    pub fn update(id: &str) -> String {
        format!("/admin/alurs/{id}")
    }

    pub fn delete(id: &str) -> String {
        format!("/admin/alurs/{id}")
    }
}

pub mod users {
    pub const LIST: &str = "/private/users";
    pub const CREATE: &str = "/admin/users";

    pub fn by_id(id: &str) -> String {
        format!("/private/users/{id}")
    }

    pub fn update(id: &str) -> String {
        format!("/admin/users/{id}")
    }

    pub fn delete(id: &str) -> String {
        format!("/admin/users/{id}")
    }

    pub fn sessions(id: &str) -> String {
        format!("/admin/users/{id}/sessions")
    }

    pub const ONLINE: &str = "/public/users/online";

    pub fn kick_session(id: &str) -> String {
        format!("/public/users/{id}/kick-session")
    }
}

pub mod profile {
    pub const IDENTIFY: &str = "/private/identify";
    pub const UPDATE: &str = "/private/profile";
    pub const CHANGE_PASSWORD: &str = "/private/profile/password";
}

pub mod satkers {
    pub const LIST: &str = "/private/satkers";
    pub const CREATE: &str = "/admin/satkers";

    pub fn by_id(id: &str) -> String {
        format!("/private/satkers/{id}")
    }

    pub fn update(id: &str) -> String {
        format!("/admin/satkers/{id}")
    }

    pub fn delete(id: &str) -> String {
        format!("/admin/satkers/{id}")
    }
}

pub mod last_positions {
    pub fn by_scenario(scenario_id: &str) -> String {
        format!("/public/last_positions/{scenario_id}")
    }
}

pub mod snapshots {
    pub fn public_get(scenario_id: &str) -> String {
        format!("/public/game-sessions/snapshot/{scenario_id}")
    }

    pub fn private_get(scenario_id: &str) -> String {
        format!("/private/game-sessions/snapshot/{scenario_id}")
    }

    pub fn upsert(scenario_id: &str) -> String {
        format!("/private/game-sessions/snapshot/{scenario_id}")
    }

    pub fn toggle(scenario_id: &str) -> String {
        format!("/private/game-sessions/snapshot/{scenario_id}/enabled")
    }

    pub fn delete(scenario_id: &str) -> String {
        format!("/private/game-sessions/snapshot/{scenario_id}")
    }
}

pub mod commands {
    pub const PUBLISH: &str = "/private/socket";
}

pub mod uploads {
    pub const IMAGE: &str = "/private/upload-image";
    pub const VIDEO: &str = "/private/upload-video";
    pub const STATIC_ROOT: &str = "/uploads";
}

#[cfg(test)]
mod tests {
    use super::{auth, categories, markers, scenarios, snapshots};

    #[test]
    fn static_paths_match_baseline() {
        assert_eq!(auth::LOGIN, "/auth/login");
        assert_eq!(scenarios::CREATE, "/admin/skenarios");
        assert_eq!(markers::CREATE, "/private/markers");
    }

    #[test]
    fn dynamic_paths_include_identifiers() {
        assert_eq!(
            categories::paginate(10, 1, "alpha"),
            "/admin/categories?pageSize=10&pageNumber=1&search=alpha"
        );
        assert_eq!(
            markers::geometry("marker-id"),
            "/private/markers/geom/marker-id"
        );
        assert_eq!(
            snapshots::toggle("scenario-id"),
            "/private/game-sessions/snapshot/scenario-id/enabled"
        );
    }
}
