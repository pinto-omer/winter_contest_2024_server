use game_components::{Entity, Float3};
use networking::{Client, ServerState};
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::{mpsc, broadcast};
use tokio::task::{self};
use tokio::time::timeout;

mod game_components;
pub mod networking;
pub async fn test_server(shutdown : broadcast::Receiver<()>) -> std::io::Result<()> {
    let tcp_listener = TcpListener::bind("127.0.0.1:8080").await?;
    let server_state = Arc::new(networking::ServerState::new(8));
    println!("server listening on port 8080");
    let clone_state = server_state.clone();
    let (tx, rx) = mpsc::channel(100);
    // handle tcp connections
    let tx_clone = tx.clone();
    let shutdown_rx = shutdown.resubscribe();
    let tcp_handle = task::spawn(async move {
        loop {
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
                    
                        task::spawn(async move {
                            if state.can_accept_new_client() {
                                let mut id = 0;
                                while state.check_client_exists(id) {
                                    id += 1;
                                }

                                let client = Client::new(id, addr, std::time::Instant::now());
                                let key = String::from(client.get_key());
                                state.add_client(id, client);
                                println!("Accepted connection from {}", addr);
                                let _ = socket.writable().await;

                                println!("key as bytes:{:?}", key.as_bytes());
                                match socket.try_write(key.as_bytes()) {
                                    Ok(_) => {}
                                    Err(_) => {
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
    let shutdown_rx = shutdown.resubscribe();
    let clone_state = server_state.clone();
    let udp_handle = task::spawn({
        async move {
            let state = clone_state;
            handle_udp(udp_listener, state, tx_clone,shutdown_rx).await;
        }
    });
    let clone_state = server_state.clone();
    let shutdown_rx = shutdown.resubscribe();

    let udp_sender_handle = task::spawn({
        async move {
            let state = clone_state;

            let _thread_error = send_updates(rx, udp_sender, state,shutdown_rx).await;
        }
    });


    tcp_handle.await?;
    println!("finished tcp handling");

    udp_handle.await?;
    println!("finished udp handling");

    udp_sender_handle.await?;
    println!("finished sending");

    Ok(())
}

async fn handle_tcp_client(
    socket: &mut TcpStream,
    id: u32,
    state: Arc<ServerState>,
    _tx: mpsc::Sender<(String, Entity)>, // future need?
    mut shutdown: broadcast::Receiver<()>
) -> Result<(), Box<dyn Error>> {
    // Handle client logic
    println!("Handling TCP client ID {}", id);

    loop {
        tokio::select! {
            _ = shutdown.recv() => {
                println!("Shutting down tcp handling...");
                break;
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_nanos(1)) => {
                match timeout(std::time::Duration::from_secs(20), socket.readable()).await {
                    Ok(val) => val,
                    Err(e) => {
                        println!("session timed out");
                        let _ = socket.shutdown().await;
                        state.remove_client(id);
                        return Err(e.into());
                    }
                }?;
        
                let mut buf = vec![0; 1024];
        
                match socket.try_read(&mut buf) {
                    Ok(n) => {
                        if n > 0 {
                            println!("TCP({id}): read {} bytes", n);
                            println!(
                                "TCP({id}): received data: {:#?}",
                                String::from_utf8_lossy(&buf[..n])
                            );
                        } else {
                            println!("connection closed");
                            state.remove_client(id);
                            return Ok(());
                        }
                    }
                    Err(ref e) if e.kind() == tokio::io::ErrorKind::WouldBlock => {
                        continue;
                    }
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
        }
    }
    
    Ok(())
}

async fn handle_udp(
    socket: Arc<UdpSocket>,
    state: Arc<ServerState>,
    tx: mpsc::Sender<(String, Entity)>,
    mut shutdown: broadcast::Receiver<()>,

) {
    let mut buf = [0; 1024];

    loop {
        tokio::select! {
            _ = shutdown.recv() => {
                println!("Shutting down udp handling...");
                break;
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_nanos(1)) => {
                if let Ok((_size, addr)) = socket.recv_from(&mut buf).await {
                    //println!("UDP Packet received from {}", addr);
                    let key = String::from_utf8_lossy(&buf[..7]);
                    if let Some(id) = state.get_client_id_by_key(&key) {
                        if let Some(address) = state.get_client_udp_addr(id) {
                            if address != addr {
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
                        let packet_type = u32::from_le_bytes(buf[7..11].try_into().unwrap());
                        //println!("UDP({id}): Key Data: {key}");
                        match packet_type {
                            1 => {
                                let player = state.get_client_player(id).unwrap();
                                let (parsed_key, parsed_entity) =
                                    player_entity_from_le_bytes(&buf[11..], player);
                                //println!(
                                //    "Received player packet. parsed key: {parsed_key}, key equals? {:?},\n entity: {:?}",
                                //    parsed_key == key,
                                //    parsed_entity
                                //);

                                if parsed_key == key {
                                    state.set_client_player(id, parsed_entity);
                                    match tx.send((parsed_key, parsed_entity)).await {
                                        Ok(_) => {}
                                        Err(e) => {
                                            println!("UDP({id}): Encountered error trying to send data to sender. {}",e.to_string());
                                        }
                                    }
                                }
                            }
                            _ => {
                                println!("Unknown packet type. Packet raw data: {:?}", &buf[7..]);
                            }
                        }

                        // Handle existing client packet

                        state.client_heartbeat(id);
                    }
                }
            }
        }
    }

}

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
            _ = tokio::time::sleep(tokio::time::Duration::from_nanos(1)) => {
                if let Some((key, player)) = rx.recv().await {
                    let mut addresses: Vec<SocketAddr> = Vec::new();
                    {
                        addresses.append(
                            &mut state
                                .get_clients()
                                .read()
                                .unwrap()
                                .values()
                                .filter(|client| client.get_key() != key)
                                .map(|client| client.get_udp_address())
                                .collect(),
                        )
                    }
                    for addr in addresses {
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
    }

    Ok(())
}

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

fn le_bytes_from_player_entity(key: &str, player: Entity) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::from(1_u32.to_le_bytes()); // player header
    let mut key = String::from(key).into_bytes();

    bytes.append(&mut key);
    for _ in [0..key.len()] {
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
    //use super::*;

    // #[test]
    // fn it_works() {

    // }
}
