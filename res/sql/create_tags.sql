CREATE TABLE "tags" (
	"id"            bigserial NOT NULL,
	"name"          text NOT NULL UNIQUE,
	"count"         bigint DEFAULT 0,
	"type"          smallint DEFAULT 0,
	CONSTRAINT "pk_tagid" PRIMARY KEY ( "id" ),
	CONSTRAINT "uq_name" UNIQUE ( "name" )
);
