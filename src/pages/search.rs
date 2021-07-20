use crate::database as db;
use crate::database::Pool as DbPool;
use crate::error::APIError;
use crate::pages::error500;

use actix_web::{web, HttpResponse};

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
	"".into()
}
fn default_page() -> u32 {
	0
}
fn default_limit() -> u32 {
	20
}
fn default_sort() -> PostSorting {
	PostSorting::DateAscending
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
	// Parse and convert the tags
	let tags: Vec<&str> =
		serde_json::from_str(&query.tags).map_err(|_| APIError::BadRequestData)?;
	if tags.len() >= 10 {
		return Err(APIError::TagLimit);
	}

	// Query database for post
	let conn = pool
		.get()
		.await
		.map_err(|e| error500("get_search:db pool", Box::new(e)))?;
	let posts = db::post::Post::select_tags(&conn, &tags, query.page, query.limit, query.sort)
		.await
		.map_err(|e| error500(&format!("get_search:select_tags {:?}", query), Box::new(e)))?;

	if posts.len() == 0 { // No posts where found
	} else { // We found some posts
	}
	// // Check to see if we actually found a post
	// match post {
	// 	Some(x) => Ok(HttpResponse::Ok()
	// 		.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
	// 		.body(serde_json::to_string(&x).map_err(|e| error500(Box::new(e)))?)),
	// 	None => Ok(HttpResponse::NotFound()
	// 		.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
	// 		.body(r#"{"error":"post not found"}"#)),
	// }
	unimplemented!()
}
