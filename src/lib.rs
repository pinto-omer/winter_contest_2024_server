//! handling all the behind-the-scenes server logic
use game_components::{Entity, Float3};
use networking::database_handler::AuthError;
use networking::{Client, ServerState};
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::{broadcast, mpsc};
use tokio::task::{self};
use tokio::time::timeout;

mod game_components;
pub mod networking;

/// function that handles all the main server functionality
///
/// # Arguments
///
/// * `shutdown` - a mutable broadcast receiver channel for receiving a graceful shutdown command from CLI
///
/// # Returns
///   -  `Ok(())`    if running finished gracefully
///   -  `Error(e)` otherwise
pub async fn main_server(mut shutdown: broadcast::Receiver<()>) -> std::io::Result<()> {
    // bind tcp listener to localhost port 8080
    // TODO: make IP and port configureable
    let tcp_listener = TcpListener::bind("127.0.0.1:8080").await?;

    // init server state in thread-safe type
    // TODO: Make max connections configureable
    let server_state = Arc::new(networking::ServerState::new(8));
    println!("server listening on port 8080");

    let clone_state = server_state.clone();

    // multi-produce single-consume transciever and receiver for collating data from multiple clients for the sender thread
    let (tx, rx) = mpsc::channel(100);
    // handle tcp connections
    let tx_clone = tx.clone(); // clone to be moved to new threads so that original transciever isn't consumed
    let (shutdown_tx, _) = broadcast::channel(1);
    let shutdown_rx = shutdown_tx.subscribe();
    let tcp_handle = task::spawn(async move {
        loop {
            // await incoming tcp connections or graceful shutdown command
            let mut shutdown = shutdown_rx.resubscribe();
            tokio::select! {
                _ = shutdown.recv() => {
                    println!("Shutting down tcp handling...");
                    break;
                }
                Ok((mut socket, addr)) = tcp_listener.accept() => {
                    let shutdown_rxx = shutdown_rx.resubscribe();
                    let tx = tx_clone.clone();
                    let state = clone_state.clone();
                    // spawn a new task that'll be in charge of handling this tcp connection
                    task::spawn(async move {
                        if state.can_accept_new_client() { // first, confirm that server didn't reached the configured max clients
                            let mut id = 0;
                            // check in serial order what the first free address is
                            // TO CONSIDER: adding this as a parameter to server state, adding client number and raising that before finding new id to prevent data races
                            while state.check_client_exists(id) {
                                id += 1;
                            }

                            // create new client with the previous id and add it to the server state
                            let client = Client::new(id, std::time::Instant::now());
                            let key = String::from(client.get_key());
                            state.add_client(id, client);
                            println!("Accepted connection from {}", addr);

                            // wait for the socket to be writeable and send the key to the client.
                            let _ = socket.writable().await;

                            println!("key as bytes:{:?}", key.as_bytes());
                            match socket.try_write(key.as_bytes()) {
                                Ok(_) => {}
                                Err(_) => { // if failed to send for any reason remove the client and close the connection
                                    state.remove_client(id);
                                    return;
                                }
                            }

                            // Handle TCP communication here
                            let _thread_error =
                                handle_tcp_client(&mut socket, id, state, tx.clone(),shutdown_rxx).await;
                        } else {
                            println!("Rejected connection from {} (server full)", addr);
                            // Optionally send a rejection message
                        }
                    });

                }
            }
        }
    });

    // handle udp connections
    let udp_socket = Arc::new(UdpSocket::bind("127.0.0.1:8080").await?);
    let udp_listener = udp_socket.clone();
    let udp_sender = udp_socket.clone();
    let tx_clone = tx.clone();
    let shutdown_rx = shutdown_tx.subscribe();
    let clone_state = server_state.clone();
    let udp_handle = task::spawn({
        // spawn a task to handle all incoming udp
        async move {
            let state = clone_state;
            let shutdown = shutdown_rx;
            handle_udp(udp_listener, state, tx_clone, shutdown).await;
        }
    });
    let clone_state = server_state.clone();
    let shutdown_rx = shutdown.resubscribe();

    let udp_sender_handle = task::spawn({
        // spawn a task to handle sending information to connected clients
        async move {
            let state = clone_state;

            let _thread_error = send_updates(rx, udp_sender, state, shutdown_rx).await;
        }
    });

    let _ = shutdown.recv().await; // wait for graceful shutdown transmission
    let _ = shutdown_tx.send(()); // pass the shutdown command along to the various threads
    tcp_handle.await?;
    println!("finished tcp handling");

    udp_handle.await?;
    println!("finished udp handling");

    udp_sender_handle.await?;
    println!("finished sending");

    Ok(())
}

/// function that handles a single tcp client's connection
///
/// # Arguments
/// - `socket` the TcpStream socket for the connection being handled
/// - `id` the client id assosciated with this socket
/// - `state` a thread-safe copy of the server state
/// - `_tx` a mpsc producer that might get used for future needs to pass messages to other threads
/// - `shutdown` a broadcast receiver for graceful shutdown messages
async fn handle_tcp_client(
    socket: &mut TcpStream,
    id: u32,
    state: Arc<ServerState>,
    _tx: mpsc::Sender<(String, Entity)>, // future need?
    mut shutdown: broadcast::Receiver<()>,
) -> Result<(), Box<dyn Error>> {
    println!("Handling TCP client ID {}", id);

    loop {
        tokio::select! {
            _ = shutdown.recv() => { // graceful shutdown branch
                println!("Shutting down tcp handling...");
                break;
            }
            // wait for messages to reach the socket. if none arrive in 20 seconds - assume timeout
            // TODO: make timeout duration configurable
            result = timeout(std::time::Duration::from_secs(20), socket.readable()) => if let Ok(_) = result {
                let mut buf = vec![0; 1024];

                match socket.try_read(&mut buf) {
                    Ok(n) => {
                        if n > 0 {
                            println!("TCP({id}): read {} bytes", n);
                            println!(
                                "TCP({id}): received data: {:#?}",
                                String::from_utf8_lossy(&buf[..n])
                            );
                            // check if client already logged in, if not - expect message to be login info and attempt login
                            if state.get_client_username(id).is_empty() {
                                let string_res = String::from_utf8_lossy(&buf);
                                let mut iter = string_res.split_whitespace();
                                if let Some(username) = iter.next()   {
                                    if let Some(password) = iter.next() {
                                        match networking::database_handler::check_user_login(username, password).await { // attempt login
                                            Ok(_) => {
                                                if state.check_user_logged_in(username) {
                                                    socket.write(&i32::to_le_bytes(4)).await?;
                                                } else {
                                                    socket.write(&i32::to_le_bytes(1)).await?;
                                                    state.set_client_username(id, username);
                                                }
                                            }
                                            Err(e) => {
                                                match e {
                                                    AuthError::DatabaseError(err) => {
                                                        println!("failed to connect to database {:?}",err.to_string());
                                                        {socket.write(&i32::to_le_bytes(2)).await?;}
                                                    }
                                                    AuthError::PasswordMismatch => {socket.write(&i32::to_le_bytes(0)).await?;}
                                                    AuthError::UserNotFound => { // if user does not exist, attempt to create it
                                                        match networking::database_handler::create_user(username,password).await {
                                                            Ok(_) =>  {socket.write(&i32::to_le_bytes(1)).await?;
                                                            state.set_client_username(id, username);}
                                                            Err(_) => {socket.write(&i32::to_le_bytes(2)).await?;}
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    } else { // did not receive username or password.
                                        socket.write(&i32::to_le_bytes(3)).await?;
                                    }
                                } else {
                                    socket.write(&i32::to_le_bytes(3)).await?;
                                }
                            } else { // handle non-login packets

                            }
                        } else { // received EOF from socket
                            println!("connection closed");
                            state.remove_client(id);
                            return Ok(());
                        }
                    }
                     // message has arrived but stream isn't ready to be read so continue
                    Err(ref e) if e.kind() == tokio::io::ErrorKind::WouldBlock => {
                        continue;
                    }
                    // elavate unexpected error
                    Err(e) => {
                        println!(
                            "TCP: received the following error from client {id}: {}",
                            e.to_string()
                        );
                        state.remove_client(id);
                        return Err(e.into());
                    }
                }
            }
            else if let Err(e) = result {
                println!("session timed out");
                let _ = socket.shutdown().await;
                state.remove_client(id);
                return Err(e.into());
            }
        }
    }

    Ok(())
}

/// function that handles all incoming udp connections
///
/// # Arguments
/// - `socket` thread-safe reference of the bound udp socket
/// - `state` thread-safe reference of the server state
/// - `tx` mpsc producer to send messages to other threads
/// - `shutdown` broadcast receiver to receive graceful shutdown requests
async fn handle_udp(
    socket: Arc<UdpSocket>,
    state: Arc<ServerState>,
    tx: mpsc::Sender<(String, Entity)>,
    mut shutdown: broadcast::Receiver<()>,
) {
    let mut buf = [0; 1024];

    loop {
        tokio::select! {
            _ = shutdown.recv() => { // graceful shutdown
                println!("Shutting down udp handling...");
                break;
            }
            // await UDP messages
            result = socket.recv_from(&mut buf) => if let Ok((_size, addr)) = result {
                    //println!("UDP Packet received from {}", addr);
                    // first portion of the message should be the 8-byte key
                    let key = String::from_utf8_lossy(&buf[..7]);
                    if let Some(id) = state.get_client_id_by_key(&key) { // check that a client with this key exists
                        if let Some(address) = state.get_client_udp_addr(id) { // check whether said client has a registered UDP address
                            if address != addr {
                                // if the registered address doesn't match the received address invalidate client
                                // to consider: just ignore the message without invalidating so it isn't abused to DC others.
                                println!(
                                    "UDP({id}):received address doesn't match saved address: {address}"
                                );
                                state.remove_client(id);
                                break;
                            }
                        } else {
                            if !state.set_client_udp_addr(id, addr) {
                                println!(
                                    "UDP({id}):failed to set client udp address due to mutex acquisition failure"
                                );
                                state.remove_client(id);
                                break;
                            } else {
                                println!("UDP({id}):registered client address {addr}");
                            }
                        }
                        // after key comes packet type
                        let packet_type = u32::from_le_bytes(buf[7..11].try_into().unwrap());
                        //println!("UDP({id}): Key Data: {key}");
                        match packet_type {
                            1 => { // player data type
                                let player = state.get_client_player(id).unwrap();
                                let (parsed_key, parsed_entity) =
                                    player_entity_from_le_bytes(&buf[11..], player);
                                //println!(
                                //    "Received player packet. parsed key: {parsed_key}, key equals? {:?},\n entity: {:?}",
                                //    parsed_key == key,
                                //    parsed_entity
                                //);

                                if parsed_key == key { // check that the key inside the player data matches the key sent with the packet
                                    state.set_client_player(id, parsed_entity); // update server state
                                    match tx.send((parsed_key, parsed_entity)).await { // send the new player data to be handled by the sending thread
                                        Ok(_) => {}
                                        Err(e) => {
                                            println!("UDP({id}): Encountered error trying to send data to sender. {}",e.to_string());
                                        }
                                    }
                                }
                                state.client_heartbeat(id); // update the UDP heartbeat
                            }
                            _ => {
                                println!("Unknown packet type. Packet raw data: {:?}", &buf[7..]);
                            }
                        }
                    }
                } else {
                    break;
                }

        }
    }
}

/// function that handles sending UDP updates to all clients with active UDP connection
///
/// # Arguments
/// - `rx` mpsc consumer to receive messages from other threads
/// - `socket` thread-safe copy of bound udp socket
/// - `shutdown` broadcast receiver for graceful shutdown
async fn send_updates(
    mut rx: mpsc::Receiver<(String, Entity)>,
    socket: Arc<UdpSocket>,
    state: Arc<ServerState>,
    mut shutdown: broadcast::Receiver<()>,
) -> Result<(), Box<dyn Error>> {
    loop {
        tokio::select! {
        _ = shutdown.recv() => {
            println!("Shutting down udp sender...");
            break;
        }
        // if the received message is player information
        result = rx.recv() => if let Some((key, player)) = result {
                let mut addresses: Vec<SocketAddr> = Vec::new();
                { // add all the UDP addresses from the server state that: arent't the sender, have had a heartbeat less than 5 seconds ago
                    addresses.append(
                        &mut state
                            .get_clients()
                            .read()
                            .unwrap()
                            .values()
                            .filter(|client| client.get_key() != key && client.duration_since_heartbeat() < 5)
                            .filter_map(|client|
                                {
                                if let Some(address) = client.get_udp_address() {
                                    Some(*address)
                                } else{ None}})
                            .collect(),
                    )
                }
                for addr in addresses { // send the player data to all the above addresses
                    println!("Sending {:?} to {addr}", player);
                    socket
                        .send_to(&le_bytes_from_player_entity(&key, player), addr)
                        .await?;
                }
            } else {
                break;
            }
        }
    }

    Ok(())
}

/// parses raw bytes packet to player entity
///
/// # Returns
/// - `(String, Entity)`
///     - String - the key extracted from the packet information
///     - Entity - the player data entity extracted from the packet information
fn player_entity_from_le_bytes(bytes: &[u8], player: Entity) -> (String, Entity) {
    let key = String::from_utf8_lossy(&bytes[..7]).try_into().unwrap();
    let new_player = Entity {
        pos: Float3(
            f32::from_le_bytes(bytes[8..12].try_into().unwrap()), // start from 8 to account for 4 byte alignment
            f32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            f32::from_le_bytes(bytes[16..20].try_into().unwrap()),
        ),
        rot: Float3(
            f32::from_le_bytes(bytes[20..24].try_into().unwrap()),
            f32::from_le_bytes(bytes[24..28].try_into().unwrap()),
            f32::from_le_bytes(bytes[28..32].try_into().unwrap()),
        ),
        scl: Float3(
            f32::from_le_bytes(bytes[32..36].try_into().unwrap()),
            f32::from_le_bytes(bytes[36..40].try_into().unwrap()),
            f32::from_le_bytes(bytes[40..44].try_into().unwrap()),
        ),
        spd: f32::from_le_bytes(bytes[44..48].try_into().unwrap()),
        max_spd: player.max_spd,
    };
    (key, new_player)
}

/// converts player entity to raw bytes (in little endian) to be sent to clients
fn le_bytes_from_player_entity(key: &str, player: Entity) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::from((1_u32).to_le_bytes()); // player header
    let mut key = String::from(key).into_bytes();
    let align = 4 - (key.len() % 4); // used for 4-bytes alignment

    bytes.append(&mut key);
    for _ in 0..align {
        // 4 Bytes alignment
        bytes.push(0);
    }

    bytes.append(&mut Vec::from(player.pos.0.to_le_bytes()));
    bytes.append(&mut Vec::from(player.pos.1.to_le_bytes()));
    bytes.append(&mut Vec::from(player.pos.2.to_le_bytes()));

    bytes.append(&mut Vec::from(player.rot.0.to_le_bytes()));
    bytes.append(&mut Vec::from(player.rot.1.to_le_bytes()));
    bytes.append(&mut Vec::from(player.rot.2.to_le_bytes()));

    bytes.append(&mut Vec::from(player.scl.0.to_le_bytes()));
    bytes.append(&mut Vec::from(player.scl.1.to_le_bytes()));
    bytes.append(&mut Vec::from(player.scl.2.to_le_bytes()));

    bytes.append(&mut Vec::from(player.spd.to_le_bytes()));
    bytes.append(&mut Vec::from(
        std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f32()
            .to_le_bytes(),
    ));
    bytes
}

#[cfg(test)]
mod tests {
    //! TO BE IMPLEMENTED
    //use super::*;

    // #[test]
    // fn it_works() {

    // }
}
