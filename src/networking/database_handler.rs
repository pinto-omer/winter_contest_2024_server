    use mysql_async;
    use mysql_async::prelude::*;
    use sha2::{Digest, Sha256};
    use rand::Rng;


	const DATABASE_URL : &str = "mysql://game_user:game_password@localhost:3306/game_db";


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
       
        let pool = mysql_async::Pool::new(DATABASE_URL);
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
        let pool = mysql_async::Pool::new(DATABASE_URL);
        let mut conn = pool.get_conn().await?;
        let mut hasher = Sha256::new();

        // let salt = Sha256::digest([ &std::time::SystemTime::now()
        // .duration_since(std::time::UNIX_EPOCH)
        // .unwrap().as_nanos().to_le_bytes(),
        // username.as_bytes()].concat());


        let mut rng = rand::rngs::OsRng;
        let mut salt = [0u8; 32]; // 32 bytes salt
        rng.fill(&mut salt);
        hasher.update(pass);
        hasher.update(salt);
        let hashed_pass = hasher.finalize();
        let query = r"INSERT INTO users values(:username,:hashed_pass,:salt,:username,0)";
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

	pub async fn add_exp(username:&str,amount:u32) -> Result<(), AuthError> {
		let pool = mysql_async::Pool::new(DATABASE_URL);
        let mut conn = pool.get_conn().await?;

		let query = r"update users set exp = exp + :amount where username=:username";
        conn.exec_drop(
            query,
            params! {
				"amount" => amount,
				"username" => username,
			}
        )
        .await?;
		Ok(())
	}
