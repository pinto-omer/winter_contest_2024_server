CREATE TABLE `users` (
   `username` varchar(30) CHARACTER SET utf8mb4 COLLATE utf8mb4_0900_ai_ci DEFAULT NULL,
   `password` binary(32) NOT NULL,
   `salt` binary(32) NOT NULL,
   `nickname` varchar(30) CHARACTER SET utf8mb4 COLLATE utf8mb4_0900_ai_ci DEFAULT NULL,
   `exp` bigint DEFAULT NULL,
   UNIQUE KEY `username` (`username`)
 );

 CREATE VIEW `user_levels` AS 
 select `users`.`username` AS `username`,
 MIN((`users`.`exp` DIV 5),50) AS `level` 
 from `users`;