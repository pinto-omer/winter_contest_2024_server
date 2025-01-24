use game_server::{networking::database_handler::{add_exp, AuthError}, test_server};
use std::io::{self, Write};
use tokio::{sync::broadcast, task};
#[tokio::main]
async fn main() {
    let (shutdown_tx, _) = broadcast::channel(1);

    let server = task::spawn(test_server(shutdown_tx.subscribe()));
    let mut last_user : String = String::new();
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
                if let Ok(receivers) = shutdown_tx.send(()) {
                    println!("sent quit message to {} receivers",receivers);
                }
                break;
            }
            line if command.starts_with("auth") => {
                let args: Vec<&str> = line.split(' ').collect();
                if args.len() != 3 {
                    println!("Unknown command: {}", command);
                } else {
                    match game_server::networking::database_handler::check_user_login(args[1], args[2]).await {
						Ok(res) => { 
                            println!("Auth success: {}",res);
                            last_user = String::from(&args[1][..]);
                        }
						Err(e) => {
                           match e {
                                AuthError::DatabaseError(err) => println!("failed to connect to database {:?}",err.to_string()),
                               AuthError::PasswordMismatch => println!("incorrect password"),
                               AuthError::UserNotFound => {
                                match game_server::networking::database_handler::create_user(args[1], args[2]).await {
                                    Ok(_) => println!("User did not exist and was created."),
                                    Err(_) => println!("User did not exist and creation has failed"),
                                }
                               }
                           }
                        }
					}
                }
            }
            line if command.starts_with("addxp") => {
                let args: Vec<&str> = line.split(' ').collect();
                if args.len() != 2 {
                    println!("addxp accepts only 1 argument, got: {}", command);
                } else if last_user.is_empty() {
                    println!("to add xp, one must first authenticate the user they wish to use.");
                } else if let Ok(amount) = args[1].parse() {
                    match add_exp(last_user.as_str(),amount).await {
                        Ok(_) => println!("added {} XP to {}",amount,last_user),
                        Err(err) => {
                            match err {
                                AuthError::DatabaseError(err) => println!("failed to connect to database {:?}",err.to_string()),
                                _ => {}
                             }
                    }
                }
                } else {
                    println!("addxp accepts only 1 integer as an argument");
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
