use game_server::{networking::database_handler::database_handler::AuthError, test_server};
use std::io::{self,Write};
use tokio::{sync::broadcast, task};
#[tokio::main]
async fn main() {

    let (shutdown_tx,_) = broadcast::channel(1);

    let server = task::spawn(test_server(shutdown_tx.subscribe()));

      // CLI loop
      loop {
        print!("> ");
        io::stdout().flush().unwrap(); // Ensure prompt is displayed
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let command = input.trim();

        match command {
            "quit" => {
                println!("Quitting...");
                // Signal the server to shut down
                let _ = shutdown_tx.send(());
                break;
            }
            line if command.starts_with("auth") => {
                let args : Vec<&str>= line.split(' ').collect();
                if args.len() != 3 {
                    println!("Unknown command: {}", command);
                } else {
					match game_server::networking::database_handler::database_handler::check_user_login(args[1], args[2]).await {
						Ok(res) =>  println!("Auth success: {}",res),
						_ => println!("Auth error"),
					}
                   
                }
            }
            _ => {
                println!("Unknown command: {}", command);
            }
        }
    }

    // Wait for the server task to complete
    let _ = server.await;
}
