pub mod database_handler {
    use mysql_async;
    use mysql_async::prelude::*;
    use sha2::{Digest, Sha256};

    #[derive(Debug)]
    pub enum AuthError {
        UserNotFound,
        PasswordMismatch,
        DatabaseError(mysql_async::Error),
    }

    impl From<mysql_async::Error> for AuthError {
        fn from(err: mysql_async::Error) -> Self {
            AuthError::DatabaseError(err)
        }
    }

    pub async fn check_user_login(username: &str, pass: &str) -> Result<bool, AuthError> {
        let database_url = "mysql://game_user:game_password@localhost:3306/game_db";
        let pool = mysql_async::Pool::new(database_url);
        let mut conn = pool.get_conn().await?;

        let query = r"SELECT salt,password from users where username = :username";

        if let Some(row) = conn.exec_first(query, params! {username}).await? {
            let (salt, stored_hash): (Vec<u8>, Vec<u8>) = mysql_async::from_row(row);
            let mut hasher = Sha256::new();
            hasher.update(pass);
            hasher.update(&salt);
            let computed_hash = hasher.finalize();

            // Compare the computed hash with the stored hash
            if computed_hash.as_slice() == stored_hash.as_slice() {
                return Ok(true);
            } else {
                return Err(AuthError::PasswordMismatch);
            }
        }

        // If no user is found
        Err(AuthError::UserNotFound)
    }

    pub async fn create_user(username: &str, pass: &str) -> Result<(), AuthError> {
        let database_url = "mysql://game_user:game_password@localhost:3306/game_db";
        let pool = mysql_async::Pool::new(database_url);
        let mut conn = pool.get_conn().await?;
        let mut hasher = Sha256::new();

        let salt = Sha256::digest([username, pass].concat());
        hasher.update(pass);
        hasher.update(salt);
        let hashed_pass = hasher.finalize();
        let query = r"INSERT INTO users values(:username,:hashed_pass,:salt,:username,0,0)";
        conn.exec_drop(
            query,
            params! {
                    "username" => username,
                "hashed_pass" => hashed_pass.as_slice(),
            "salt" => salt.as_slice()},
        )
        .await?;

        Ok(())
    }
}
