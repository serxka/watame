CREATE TABLE "tags" (
	"id"            bigserial NOT NULL,
	"name"          text NOT NULL UNIQUE,
	"count"         bigint DEFAULT 0,
	"type"          int DEFAULT 0,
	CONSTRAINT "pk_tagid" PRIMARY KEY ( "id" )
);
