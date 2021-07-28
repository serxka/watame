use crate::database::{tag::Tag, Pool as DbPool};
use crate::{error::APIError, try500};

use actix_web::{http::header, web, HttpResponse};

#[derive(serde::Deserialize)]
pub struct TagInfoQuery {
	#[serde(rename = "t")]
	name: String,
}

pub async fn get_info(
	query: web::Query<TagInfoQuery>,
	pool: web::Data<DbPool>,
) -> Result<HttpResponse, APIError> {
	// Query database for tag
	let conn = try500!(pool.get().await, "get_tag:db pool");
	let tag = try500!(
		Tag::select_tag_name(&conn, &query.name).await,
		"get_tag:select_tag_name {}",
		query.name
	);

	// Check to see if we actually found a tag
	match tag {
		Some(x) => Ok(HttpResponse::Ok()
			.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
			.body(try500!(serde_json::to_string(&x), "get_tag:json serialize"))),
		None => Ok(HttpResponse::NotFound()
			.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
			.body(r#"{"error":"tag not found"}"#)),
	}
}
