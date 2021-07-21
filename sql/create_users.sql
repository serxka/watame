CREATE TABLE "users"
(
    "id"          bigserial NOT NULL,
    "name"        varchar(24) NOT NULL,
    "email"       text NULL,
    "pass"        bytea NOT NULL,
    "picture"     text NOT NULL DEFAULT '/s/pfp/default.png',
    CONSTRAINT "PK_userid" PRIMARY KEY ( "id" ),
    UNIQUE (name)
);

INSERT INTO users (id, name, pass) VALUES (0, 'wadmin', 'password!');
