use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
pub struct Client {
    id: u32,
    address: SocketAddr,
    last_heartbeat: std::time::Instant,
    key: String,
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

    pub fn get_client_id_by_addr(&self, addr: SocketAddr) -> Option<u32> {
        self.clients
            .lock()
            .unwrap()
            .values()
            .filter(|c| c.address == addr)
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
}

impl Client {
    pub fn new(id: u32, address: SocketAddr, last_heartbeat: std::time::Instant) -> Self {
        let mut key = String::from("client");
        key.push_str(&id.to_string()[..]);
        Client {
            id,
            address,
            last_heartbeat,
            key,
        }
    }
    pub fn get_key(&self) -> &str {
        &self.key[..]
    }
}
