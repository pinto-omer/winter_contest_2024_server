
pub mod database_handler {
    use params::Params;
    use sha2::{Sha256, Digest};
    use mysql_async;
    use mysql_async::prelude::*;
    use std::error::Error;

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



 pub async fn check_user_login (username: &str, pass: &str) -> Result<bool,AuthError>{
    
    let database_url = "mysql://game_user:game_password@localhost:3306/database_name";
    let pool = mysql_async::Pool::new(database_url);
    let mut conn = pool.get_conn().await?;


    
    let query = r"SELECT salt,password from users where username = :username";

    if let Some(row) = conn.exec_first(query, params!{username}).await? {
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

    // If no user is found, return false
    Err(AuthError::UserNotFound)
 }   
}