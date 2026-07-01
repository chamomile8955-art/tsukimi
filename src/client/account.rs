use serde::{
    Deserialize,
    Serialize,
};

use crate::ui::provider::descriptor::VecSerialize;
use url::Url;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]

pub enum ServerType {
    #[default]
    Emby = 0,
    Jellyfin = 1,
}

impl ServerType {
    pub fn index(self) -> u32 {
        self as u32
    }

    pub fn from_index(index: u32) -> Self {
        match index {
            1 => Self::Jellyfin,
            _ => Self::Emby,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
pub struct ServerRoute {
    pub name: String,
    pub url: String,
}

impl ServerRoute {
    pub fn validated(name: &str, url: &str) -> Result<Self, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Route name cannot be empty".to_string());
        }

        let mut parsed =
            Url::parse(url.trim()).map_err(|error| format!("Invalid route URL: {error}"))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err("Route URL must use http:// or https://".to_string());
        }
        if parsed.host_str().is_none() {
            return Err("Route URL must contain a host".to_string());
        }
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err("Route URL must not contain credentials".to_string());
        }
        if parsed.query().is_some() || parsed.fragment().is_some() {
            return Err("Route URL must not contain a query or fragment".to_string());
        }

        if !parsed.path().ends_with('/') {
            let path = format!("{}/", parsed.path());
            parsed.set_path(&path);
        }

        Ok(Self {
            name: name.to_string(),
            url: parsed.to_string().trim_end_matches('/').to_string(),
        })
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Default)]
pub struct Account {
    pub servername: String,
    #[serde(default)]
    pub server: String,
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub port: String,
    pub user_id: String,
    pub access_token: String,
    pub server_type: Option<ServerType>,
    #[serde(default)]
    pub routes: Vec<ServerRoute>,
    #[serde(default)]
    pub default_route: Option<String>,
    #[serde(default)]
    pub selected_route: Option<String>,
}

impl Account {
    pub fn normalize_routes(&mut self) -> bool {
        let mut changed = false;

        if self.routes.is_empty()
            && let Some(route) = self.legacy_route()
        {
            self.routes.push(route);
            changed = true;
        }

        for route in &mut self.routes {
            if let Ok(normalized) = ServerRoute::validated(&route.name, &route.url)
                && *route != normalized
            {
                *route = normalized;
                changed = true;
            }
        }

        let default_exists = self
            .default_route
            .as_ref()
            .is_some_and(|name| self.routes.iter().any(|route| &route.name == name));
        if !default_exists {
            let fallback = self.routes.first().map(|route| route.name.clone());
            if self.default_route != fallback {
                self.default_route = fallback;
                changed = true;
            }
        }

        let selected_exists = self
            .selected_route
            .as_ref()
            .is_some_and(|name| self.routes.iter().any(|route| &route.name == name));
        if !selected_exists {
            let fallback = self
                .default_route
                .clone()
                .or_else(|| self.routes.first().map(|route| route.name.clone()));
            if self.selected_route != fallback {
                self.selected_route = fallback;
                changed = true;
            }
        }

        let legacy_before = (self.server.clone(), self.port.clone());
        self.sync_legacy_address();
        changed || legacy_before != (self.server.clone(), self.port.clone())
    }

    pub fn active_route(&self) -> Option<&ServerRoute> {
        self.selected_route
            .as_ref()
            .and_then(|name| self.routes.iter().find(|route| &route.name == name))
            .or_else(|| {
                self.default_route
                    .as_ref()
                    .and_then(|name| self.routes.iter().find(|route| &route.name == name))
            })
            .or_else(|| self.routes.first())
    }

    pub fn select_route(&mut self, route_name: &str) -> Result<(), String> {
        let route = self
            .routes
            .iter()
            .find(|route| route.name == route_name)
            .ok_or_else(|| format!("Route not found: {route_name}"))?;
        ServerRoute::validated(&route.name, &route.url)?;
        self.selected_route = Some(route.name.clone());
        self.sync_legacy_address();
        Ok(())
    }

    fn legacy_route(&self) -> Option<ServerRoute> {
        if self.server.trim().is_empty() {
            return None;
        }

        let mut url = Url::parse(self.server.trim()).ok()?;
        if url.port().is_none() && !self.port.trim().is_empty() {
            let port = self.port.parse::<u16>().ok()?;
            url.set_port(Some(port)).ok()?;
        }
        ServerRoute::validated("默认线路", url.as_str()).ok()
    }

    fn sync_legacy_address(&mut self) {
        let Some(route) = self.active_route() else {
            return;
        };
        let Ok(mut url) = Url::parse(&route.url) else {
            return;
        };
        let port = url.port_or_known_default().map(|port| port.to_string());
        let _ = url.set_port(None);
        self.server = url.to_string().trim_end_matches('/').to_string();
        self.port = port.unwrap_or_default();
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub struct Accounts {
    pub accounts: Vec<Account>,
}

impl VecSerialize<Account> for Vec<Account> {
    fn to_string(&self) -> String {
        serde_json::to_string(&self).expect("Failed to serialize Vec<Descriptor>")
    }
}
