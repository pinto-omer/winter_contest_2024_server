use game_server::test_server;
#[tokio::main]
async fn main() {
    match test_server().await {
        Ok(_) => {}
        Err(_) => {}
    };
}
