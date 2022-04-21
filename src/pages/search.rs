use crate::database::{pg, post::Post, Pool as DbPool};
use crate::{error::APIError, try500};

use actix_web::{http::header, web, HttpResponse};

#[derive(Debug, Copy, Clone, serde::Deserialize)]
pub enum PostSorting {
	#[serde(rename = "da")]
	DateAscending,
	#[serde(rename = "dd")]
	DateDescending,
	#[serde(rename = "va")]
	VoteAscending,
	#[serde(rename = "vd")]
	VoteDescending,
}

fn default_tags() -> String {
	"[]".into()
}
fn default_page() -> u32 {
	0
}
fn default_limit() -> u32 {
	20
}
fn default_sort() -> PostSorting {
	PostSorting::DateDescending
}

#[derive(Debug, serde::Deserialize)]
pub struct SearchPostQuery {
	#[serde(alias = "t", default = "default_tags")]
	tags: String,
	#[serde(alias = "p", default = "default_page")]
	page: u32,
	#[serde(alias = "l", default = "default_limit")]
	limit: u32,
	#[serde(alias = "s", default = "default_sort")]
	sort: PostSorting,
}

pub async fn get_search(
	query: web::Query<SearchPostQuery>,
	pool: web::Data<DbPool>,
) -> Result<HttpResponse, APIError> {
	let mut tags: Vec<&str> =
		serde_json::from_str(&query.tags).map_err(|_| APIError::BadRequestData)?;
	for i in 0..tags.len() {
		tags[i] = tags[i].trim();
		if tags[i].is_empty() {
			tags.remove(i);
		}
	}
	if tags.len() > 10 {
		return Err(APIError::TagLimit);
	}
	if query.limit > 50 {
		return Err(APIError::PageSize);
	}

	// Query database for post
	let conn = try500!(pool.get().await, "get_search:db pool");
	let posts = try500!(
		Post::select_fulltext_tags::<pg::Client>(&conn, &tags, query.page, query.limit, query.sort)
			.await,
		"get_search:select_fulltext_tags {:?}",
		query
	);

	if posts.len() == 0 {
		Ok(HttpResponse::NotFound()
			.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
			.body(r#"{"error":"no posts found"}"#))
	} else {
		Ok(HttpResponse::Ok()
			.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
			.body(serde_json::to_string(&posts).unwrap()))
	}
}

pub async fn get_random_post(pool: web::Data<DbPool>) -> Result<HttpResponse, APIError> {
	// Query database for post
	let conn = try500!(pool.get().await, "get_search:db pool");
	let post = try500!(
		Post::select_post_random::<pg::Client>(&conn).await,
		"get_random_post:select_post_random"
	);

	// Check to see if we actually found a post
	match post {
		Some(x) => Ok(HttpResponse::Ok()
			.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
			.body(serde_json::to_string(x.as_full()).unwrap())),
		None => Ok(HttpResponse::NotFound()
			.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
			.body(r#"{"error":"no posts found"}"#)),
	}
}
