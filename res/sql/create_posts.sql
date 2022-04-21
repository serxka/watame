CREATE TYPE imgext AS ENUM (
    'Bmp', 'Gif', 'Jpg', 'Png', 'Tiff', 'Webp'
);

CREATE TYPE rating AS ENUM (
    'Safe', 'Sketchy', 'Explicit'
);

CREATE TABLE "posts"
(
    "id"            bigserial NOT NULL,
    "poster"        integer NOT NULL,
    "tag_vector"    tsvector NOT NULL,
    "create_date"   timestamp with time zone NOT NULL DEFAULT now(),
    "modified_date" timestamp with time zone NOT NULL DEFAULT now(),
    "description"   text,
    "rating"        rating NOT NULL DEFAULT 'Sketchy',
    "score"         integer NOT NULL DEFAULT 0,
    "views"         integer NOT NULL DEFAULT 0,
    "source"        text,
    "filename"      text NOT NULL,
    "path"          text NOT NULL,
    "ext"           imgext NOT NULL,
    "size"          integer NOT NULL,
    "width"         integer NOT NULL,
    "height"        integer NOT NULL,
    "is_deleted"    boolean NOT NULL DEFAULT false,
    CONSTRAINT "pk_postid" PRIMARY KEY ( "id" ),
    CONSTRAINT "fk_poster" FOREIGN KEY ( "poster" ) REFERENCES "users" ( "id" )
);

CREATE INDEX "idx_posts_create_date" ON "posts" USING btree (create_date);
CREATE INDEX "idx_posts_tag_vector" ON "posts" USING gin (tag_vector);
CREATE INDEX "idx_posts_is_deleted" ON "posts" USING btree (is_deleted);
