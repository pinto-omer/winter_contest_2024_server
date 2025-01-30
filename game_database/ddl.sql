CREATE TABLE `game_db`.`users` (
   `username` varchar(30) CHARACTER SET utf8mb4 COLLATE utf8mb4_0900_ai_ci DEFAULT NULL,
   `password` binary(32) NOT NULL,
   `salt` binary(32) NOT NULL,
   `nickname` varchar(30) CHARACTER SET utf8mb4 COLLATE utf8mb4_0900_ai_ci DEFAULT NULL,
   `exp` bigint DEFAULT NULL,
   UNIQUE KEY `username` (`username`)
 );

 CREATE VIEW `game_db`.`user_levels` AS 
 select `users`.`username` AS `username`,
 (`users`.`exp` DIV 5) AS `level` 
 from `game_db`.`users`;
 