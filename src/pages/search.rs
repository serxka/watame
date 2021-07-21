use crate::database as db;
use crate::database::Pool as DbPool;
use crate::error::APIError;
use crate::pages::error500;

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
		if tags[i].chars().any(|c| matches!(c, ' ' | '|' | '(' | ')')) {
			return Err(APIError::BadTags);
		}
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
	let conn = pool
		.get()
		.await
		.map_err(|e| error500("get_search:db pool", Box::new(e)))?;
	let posts = db::post::Post::select_tags(&conn, &tags, query.page, query.limit, query.sort)
		.await
		.map_err(|e| error500(&format!("get_search:select_tags {:?}", query), Box::new(e)))?;

	if posts.len() == 0 {
		Ok(HttpResponse::NotFound()
			.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
			.body(r#"{"error":"no posts found"}"#))
	} else {
		Ok(HttpResponse::Ok()
			.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
			.body(
				serde_json::to_vec(&posts)
					.map_err(|e| error500("get_search:json serialize", Box::new(e)))?,
			))
	}
}

pub async fn get_random_post(pool: web::Data<DbPool>) -> Result<HttpResponse, APIError> {
	// Query database for post
	let conn = pool
		.get()
		.await
		.map_err(|e| error500("get_random_post:db pool", Box::new(e)))?;
	let post = db::post::Post::select_post_random(&conn)
		.await
		.map_err(|e| error500("get_random_post:select_post_random", Box::new(e)))?;

	// Check to see if we actually found a post
	match post {
		Some(x) => Ok(HttpResponse::Ok()
			.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
			.body(
				serde_json::to_string(&x)
					.map_err(|e| error500("get_random_post:json serialize", Box::new(e)))?,
			)),
		None => Ok(HttpResponse::NotFound()
			.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
			.body(r#"{"error":"no posts found"}"#)),
	}
}
