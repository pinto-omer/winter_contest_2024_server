CREATE TABLE `game_db`.`users` (
   `username` varchar(30) CHARACTER SET utf8mb4 COLLATE utf8mb4_0900_ai_ci NOT NULL PRIMARY KEY,
   `password` binary(32) NOT NULL,
   `salt` binary(32) NOT NULL,
   `nickname` varchar(30) CHARACTER SET utf8mb4 COLLATE utf8mb4_0900_ai_ci DEFAULT NULL,
   `exp` int DEFAULT 0
 );

 CREATE VIEW `game_db`.`user_levels` AS 
 select `users`.`username` AS `username`,
 (`users`.`exp` DIV 5) AS `level` 
 from `game_db`.`users`;
 