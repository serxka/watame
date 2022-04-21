CREATE TYPE perms AS ENUM (
    'Guest', 'User', 'Moderator', 'Admin'
);

CREATE TABLE "users"
(
    "id"          serial NOT NULL,
    "name"        varchar(24) NOT NULL,
    "email"       text NULL,
    "pass"        text NOT NULL,
    "picture"     text NOT NULL DEFAULT '/s/pfp/default.png',
    "permissions" perms NOT NULL DEFAULT 'User',
    CONSTRAINT "PK_userid" PRIMARY KEY ( "id" ),
    UNIQUE (name)
);

INSERT INTO users (id, name, pass, permissions) VALUES (0, 'wadmin', 'password!', 'Admin');
