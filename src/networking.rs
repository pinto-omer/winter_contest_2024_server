use crate::game_components::Float3;

use super::game_components::Entity;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

pub mod database_handler;

pub struct Client {
    id: u32,
    username: String,
    udp_address: Option<SocketAddr>,
    last_heartbeat: std::time::Instant,
    key: String,
    player: Entity,
}

pub struct ServerState {
    clients: Arc<RwLock<HashMap<u32, Client>>>, // Shared client list
    current_connections: AtomicUsize,           // Current connection count
    max_connections: usize,                     // Maximum allowed connections
}

impl ServerState {
    pub fn new(max_connections: usize) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            current_connections: AtomicUsize::new(0),
            max_connections,
        }
    }
    pub fn get_clients(&self) -> Arc<RwLock<HashMap<u32, Client>>> {
        self.clients.clone()
    }

    pub fn can_accept_new_client(&self) -> bool {
        self.current_connections.load(Ordering::Relaxed) < self.max_connections
    }

    pub fn check_client_exists(&self, id: u32) -> bool {
        if let Some(_) = self.clients.read().unwrap().get(&id) {
            true
        } else {
            false
        }
    }

    pub fn get_client_id_by_key(&self, key: &str) -> Option<u32> {
        self.clients
            .read()
            .unwrap()
            .values()
            .filter(|c| c.key == key)
            .map(|client| client.id)
            .next()
    }

    pub fn add_client(&self, id: u32, client: Client) -> bool {
        if self.can_accept_new_client() {
            self.clients.write().unwrap().insert(id, client);
            self.current_connections.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    pub fn remove_client(&self, id: u32) {
        if self.clients.write().unwrap().remove(&id).is_some() {
            self.current_connections.fetch_sub(1, Ordering::Relaxed);
        }
    }

    pub fn client_heartbeat(&self, id: u32) {
        if let Some(client) = self.clients.write().unwrap().get_mut(&id) {
            client.update_heartbeat();
        }
    }

    pub fn get_client_player(&self, id: u32) -> Option<Entity> {
        if let Some(client) = self.clients.read().unwrap().get(&id) {
            Some(client.get_player())
        } else {
            None
        }
    }

    pub fn set_client_player(&self, id: u32, player: Entity) -> bool {
        if let Some(client) = self.clients.write().unwrap().get_mut(&id) {
            client.set_player(player);
            true
        } else {
            false
        }
    }

    pub fn get_client_udp_addr(&self, id: u32) -> Option<SocketAddr> {
        self.clients.read().unwrap().get(&id).unwrap().udp_address
    }
    pub fn get_client_username(&self, id:u32) -> String {
        String::from(self.clients.read().unwrap().get(&id).unwrap().username.as_str())
    }

    pub fn set_client_username(&self, id: u32, username: &str) {
        if let Ok(mut clients) = self.clients.write() {
            let client = clients.get_mut(&id).unwrap();
            client.set_username(username);
        }
    }
    pub fn set_client_udp_addr(&self, id: u32, udp_address: SocketAddr) -> bool {
        match self.clients.write() {
            Ok(mut clients) => {
                let client = clients.get_mut(&id).unwrap();
                match client.udp_address {
                    Some(_) => false,
                    None => {
                        client.udp_address = Some(udp_address);
                        true
                    }
                }
            }
            Err(_) => {
                panic!("could not acquire lock");
            }
        }
    }

    pub fn reset_client_udp_address(&self, id: u32) {
        if let Ok(mut clients) = self.clients.write() {
            clients.get_mut(&id).unwrap().clear_udp();
        } else {
            panic!("failed to acquire lock to reset client {id}'s udp address")
        }
    }

    pub fn check_user_logged_in(&self, username: &str) -> bool {
        self.clients
            .read()
            .unwrap()
            .values()
            .filter(|c| c.username == username)
            .count() > 0
    }
}

impl Client {
    pub fn new(id: u32, last_heartbeat: std::time::Instant) -> Self {
        let mut key = String::from("client");
        key.push_str(&id.to_string()[..]);
        Client {
            id,
            username: String::from(""),
            last_heartbeat,
            key,
            udp_address: None,
            player: Entity {
                pos: Float3(0.0, 0.0, 0.0),
                rot: Float3(0.0, 0.0, 0.0),
                spd: 0.0,
                scl: Float3(1.0, 1.0, 1.0),
                max_spd: 10.0,
            },
        }
    }
    pub fn get_key(&self) -> &str {
        &self.key[..]
    }
    pub fn get_udp_address(&self) -> Option<SocketAddr> {
        self.udp_address
            .clone()
    }
    fn update_heartbeat(&mut self) {
        self.last_heartbeat = std::time::Instant::now();
    }

    fn get_player(&self) -> Entity {
        self.player
    }

    fn set_player(&mut self, player: Entity) {
        self.player = player;
    }
    pub fn get_username(&self) -> &str {
        self.username.as_str()
    }

    fn set_username(&mut self, username: &str) {
        self.username = String::from(username);
    }
    fn clear_udp(&mut self) {
        self.udp_address = None;
    }

    pub fn duration_since_heartbeat(&self) -> u64 {
        match std::time::Instant::now().checked_duration_since(self.last_heartbeat) {
            Some(dur) => {dur.as_secs()},
            None => {u64::MAX},
        }
    }
}
