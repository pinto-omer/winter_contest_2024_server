use networking::{Client, ServerState};
use std::error::Error;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::task;
use tokio::time::error::Elapsed;
use tokio::time::timeout;

mod networking;

pub async fn test_server() -> std::io::Result<()> {
    let tcp_listener = TcpListener::bind("127.0.0.1:8080").await?;
    let server_state = Arc::new(networking::ServerState::new(8));
    println!("server listening on port 8080");
    let clone_state = server_state.clone();

    // handle tcp connections
    let tcp_handle = task::spawn(async move {
        loop {
            let state = clone_state.clone();
            if let Ok((mut socket, addr)) = tcp_listener.accept().await {
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
                        handle_tcp_client(&mut socket, id, state).await;
                    } else {
                        println!("Rejected connection from {} (server full)", addr);
                        // Optionally send a rejection message
                    }
                });
            }
        }
    });

    // handle udp connections
    let udp_listener = UdpSocket::bind("127.0.0.1:8080").await?;

    let clone_state = server_state.clone();
    let udp_handle = task::spawn({
        async move {
            let state = clone_state.clone();
            handle_udp(udp_listener, state).await;
        }
    });

    tcp_handle.await?;
    udp_handle.await?;
    Ok(())
}

async fn handle_tcp_client(
    socket: &mut TcpStream,
    id: u32,
    state: Arc<ServerState>,
) -> Result<(), Box<dyn Error>> {
    // Handle client logic
    println!("Handling TCP client ID {}", id);

    loop {
        let ready = match timeout(std::time::Duration::from_secs(20), socket.readable()).await {
            Ok(val) => val,
            Err(e) => {
                println!("session timed out");
                let _ = socket.shutdown().await;
                state.remove_client(id);
                return Err(e.into());
            }
        };

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
                return Err(e.into());
            }
        }
    }
}

async fn handle_udp(socket: UdpSocket, state: Arc<ServerState>) {
    let mut buf = [0; 1024];

    loop {
        if let Ok((size, addr)) = socket.recv_from(&mut buf).await {
            println!("UDP Packet received from {}", addr);
            if let Some(id) = state.get_client_id_by_addr(addr) {
                // Handle existing client packet
                println!(", ID:{}", id);
                println!("Packet data: {:#?}", buf);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // #[test]
    // fn it_works() {

    // }
}
