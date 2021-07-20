CREATE TYPE extension AS ENUM (
    'bmp', 'gif', 'jpeg', 'png', 'tiff', 'webp'
);

CREATE TABLE "posts"
(
    "id"           bigserial NOT NULL,
    "upload_date"  timestamp with time zone DEFAULT now(),
    "filename"     text NOT NULL,
    "path"         char(4) NOT NULL,
    "ext"          text NOT NULL,
    "size"         integer NOT NULL,
    "width"        integer NOT NULL,
    "height"       integer NOT NULL,
    "description"  text DEFAULT 'No Description Provided',
    "tags"         text NOT NULL,
    "score"        integer DEFAULT 0,
    "poster"       bigint NOT NULL,
    CONSTRAINT "PK_postid" PRIMARY KEY ( "id" ),
    CONSTRAINT "FK_poster" FOREIGN KEY ( "poster" ) REFERENCES "users" ( "id" )
);
