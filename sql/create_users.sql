CREATE TABLE "users"
(
    "id"          bigserial NOT NULL,
    "name"        varchar(24) NOT NULL,
    "email"       text NOT NULL,
    "pass"        char(60) NOT NULL,
    "picture_dir" char(4) NOT NULL,
    CONSTRAINT "PK_userid" PRIMARY KEY ( "id" )
);

INSERT INTO users VALUES 
    ('0', 'serxka', 'serxka@example.com', 'password!', '0001')
;