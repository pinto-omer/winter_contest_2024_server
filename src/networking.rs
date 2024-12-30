use crate::game_components::Float3;

use super::game_components::Entity;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub struct Client {
    id: u32,
    _address: SocketAddr,
    last_heartbeat: std::time::Instant,
    key: String,
    player: Entity,
}

pub struct ServerState {
    clients: Arc<Mutex<HashMap<u32, Client>>>, // Shared client list
    current_connections: AtomicUsize,          // Current connection count
    max_connections: usize,                    // Maximum allowed connections
}

impl ServerState {
    pub fn new(max_connections: usize) -> Self {
        Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
            current_connections: AtomicUsize::new(0),
            max_connections,
        }
    }

    pub fn can_accept_new_client(&self) -> bool {
        self.current_connections.load(Ordering::Relaxed) < self.max_connections
    }

    pub fn check_client_exists(&self, id: u32) -> bool {
        if let Some(_) = self.clients.lock().unwrap().get(&id) {
            true
        } else {
            false
        }
    }

    pub fn get_client_id_by_key(&self, key: &str) -> Option<u32> {
        self.clients
            .lock()
            .unwrap()
            .values()
            .filter(|c| c.key == key)
            .map(|client| client.id)
            .next()
    }

    pub fn add_client(&self, id: u32, client: Client) -> bool {
        if self.can_accept_new_client() {
            self.clients.lock().unwrap().insert(id, client);
            self.current_connections.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    pub fn remove_client(&self, id: u32) {
        if self.clients.lock().unwrap().remove(&id).is_some() {
            self.current_connections.fetch_sub(1, Ordering::Relaxed);
        }
    }

    pub fn client_heartbeat(&self, id: u32) {
        if let Some(client) = self.clients.lock().unwrap().get_mut(&id) {
            client.update_heartbeat();
        }
    }

    pub fn get_client_player(&self, id: u32) -> Option<Entity> {
        if let Some(client) = self.clients.lock().unwrap().get_mut(&id) {
            Some(client.get_player())
        } else {
            None
        }
    }

    pub fn set_client_player(&self, id: u32, player: Entity) -> bool {
        if let Some(client) = self.clients.lock().unwrap().get_mut(&id) {
            client.set_player(player);
            true
        } else {
            false
        }
    }
}

impl Client {
    pub fn new(id: u32, address: SocketAddr, last_heartbeat: std::time::Instant) -> Self {
        let mut key = String::from("client");
        key.push_str(&id.to_string()[..]);
        Client {
            id,
            _address: address,
            last_heartbeat,
            key,
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

    fn update_heartbeat(&mut self) {
        self.last_heartbeat = std::time::Instant::now();
    }

    fn get_player(&self) -> Entity {
        self.player
    }

    fn set_player(&mut self, player: Entity) {
        self.player = player;
    }
}
