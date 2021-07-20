use std::path::PathBuf;

use crate::database as db;
use crate::database::Pool as DbPool;
use crate::error::APIError;
use crate::pages::error500;
use crate::settings::RunSettings;

use actix_multipart::Multipart;
use actix_web::{http::header, web, HttpResponse};
use futures::{StreamExt, TryStreamExt};

fn image_path(id: i64) -> String {
	let mut x = 0u16;
	x ^= (id >> 0) as u16;
	x ^= (id >> 16) as u16;
	x ^= (id >> 32) as u16;
	x ^= (id >> 48) as u16;
	format!("{:04x}", x - 2)
}

const UPLOAD_PAGE_HTML: &'static str = r#"
<!DOCTYPE html>
<html>
<head>
	<title>Upload</title>
</head>
<body>
<form method="POST" action="/post" enctype="multipart/form-data">
	<input type="file" name="image">
	<input type="textarea" name="data">
	<button type="submit">Submit</button>
</form>
</body>
</html>
"#;

pub async fn get_upload() -> Result<HttpResponse, APIError> {
	Ok(HttpResponse::Ok()
		.append_header((header::CONTENT_TYPE, "text/html; charset=utf-8"))
		.body(UPLOAD_PAGE_HTML))
}

#[derive(serde::Deserialize)]
pub struct IdPostQuery {
	id: i64,
}

pub async fn get_post(
	query: web::Query<IdPostQuery>,
	pool: web::Data<DbPool>,
) -> Result<HttpResponse, APIError> {
	// Verify we haven't been given a negative ID
	if query.id < 0 {
		return Err(APIError::BadRequestData);
	}

	// Query database for post
	let conn = pool
		.get()
		.await
		.map_err(|e| error500("get_post:db pool", Box::new(e)))?;
	let post = db::post::Post::select_post(&conn, query.id)
		.await
		.map_err(|e| error500(&format!("get_post:select_id {}", query.id), Box::new(e)))?;

	// Check to see if we actually found a post
	match post {
		Some(x) => Ok(HttpResponse::Ok()
			.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
			.body(
				serde_json::to_string(&x)
					.map_err(|e| error500("get_post:json serialize", Box::new(e)))?,
			)),
		None => Ok(HttpResponse::NotFound()
			.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
			.body(r#"{"error":"post not found"}"#)),
	}
}

pub async fn delete_post(
	query: web::Query<IdPostQuery>,
	pool: web::Data<DbPool>,
) -> Result<HttpResponse, APIError> {
	// Verify we haven't been given a negative ID
	if query.id < 0 {
		return Err(APIError::BadRequestData);
	}

	// Query database for post
	let conn = pool
		.get()
		.await
		.map_err(|e| error500("delete_post:db pool", Box::new(e)))?;
	let post = db::post::Post::select_id_poster(&conn, query.id)
		.await
		.map_err(|e| error500(&format!("delete_post:select_id {}", query.id), Box::new(e)))?;

	// if it exists and we are the owner we can delete it
	match post {
		Some(_post) => {
			db::post::Post::delete_post(&conn, query.id)
				.await
				.map_err(|e| {
					error500(
						&format!("delete_post:delete_post {}", query.id),
						Box::new(e),
					)
				})?;
			Ok(HttpResponse::Ok()
				.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
				.body(r#"{"success":"post deleted"}"#))
		}
		None => Ok(HttpResponse::NotFound()
			.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
			.body(r#"{"error":"post not found"}"#)),
	}
}

pub async fn post_upload(
	payload: Multipart,
	pool: web::Data<DbPool>,
	settings: web::Data<RunSettings>,
) -> Result<HttpResponse, APIError> {
	let (image_data, filename, json) = process_multipart_image(payload).await?;

	// Load image into memory for thumbnail/info/hashing
	let image_type = image::guess_format(&image_data).map_err(|_| APIError::MimeType)?;
	let image = image::load_from_memory_with_format(&image_data, image_type)
		.map_err(|_| APIError::BadRequestData)?;

	// Image metadata
	let dimensions = image::GenericImageView::dimensions(&image);
	let file_size = image_data.len() as u32;

	// Generate thumb
	let thumbnail = image.thumbnail(320, 320);

	// Items from JSON description
	let description = json
		.get("description")
		.ok_or(APIError::BadRequestData)?
		.as_str()
		.ok_or(APIError::BadRequestData)?;
	let tags_json = json
		.get("tags")
		.ok_or(APIError::BadRequestData)?
		.as_array()
		.ok_or(APIError::BadRequestData)?;

	// Check that tags are valid and add them to an array
	let mut tags = Vec::new();
	for tag in tags_json {
		let s = tag.as_str().ok_or(APIError::BadRequestData)?.trim();
		if s.chars()
			.any(|c| matches!(c, ' ' | '+' | '!' | '|' | '(' | ')'))
		{
			return Err(APIError::BadTags);
		}
		tags.push(s);
	}

	let new_post = db::post::NewPost {
		filename: &filename,
		ext: image_type.into(),
		path: "0000",
		size: file_size as i32,
		dimensions: (dimensions.0 as i32, dimensions.1 as i32),
		description: description,
		tags: &tags,
		poster: 0,
	};

	let conn = pool
		.get()
		.await
		.map_err(|e| error500("post_upload:db pool", Box::new(e)))?;
	let mut post = new_post.insert_into(&conn).await.map_err(|e| {
		error500(
			&format!("post_upload:insert_into {:?}", new_post),
			Box::new(e),
		)
	})?;
	let subfolder = image_path(post.id);
	post.update_path(&conn, &subfolder)
		.await
		.map_err(|e| error500("post_upload:update_path", Box::new(e)))?;

	// File path for the primary image
	let img_path: PathBuf = [
		&settings.storage_root,
		"img",
		&subfolder,
		&format!("{}-{}", post.id, post.filename),
	]
	.iter()
	.collect();
	// File path for the smaller thumbnail
	let tmb_path: PathBuf = [
		&settings.storage_root,
		"tmb",
		&subfolder,
		&format!("{}-thumbnail.jpg", post.id),
	]
	.iter()
	.collect();

	println!("{}", img_path.display());
	println!("{}", tmb_path.display());

	// Async fs write the main image as it's already encoded
	let img = async_std::fs::write(&img_path, &image_data);
	// We have to first encoder the thumbnail as a Jpeg before we can write it
	let mut tmb_data = Vec::new();
	thumbnail
		.write_to(&mut tmb_data, image::ImageOutputFormat::Jpeg(70))
		.map_err(|e| error500("json encode", Box::new(e)))?;
	let tmb = async_std::fs::write(&tmb_path, &tmb_data);

	// Take these two futures and wait on them
	let (img, tmb) = futures::join!(img, tmb);
	img.map_err(|e| error500(&format!("image write {}", img_path.display()), Box::new(e)))?;
	tmb.map_err(|e| error500(&format!("thumb write {}", tmb_path.display()), Box::new(e)))?;

	Ok(HttpResponse::Ok()
		.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
		.body(
			serde_json::to_string(&post)
				.map_err(|e| error500("upload_post:json serialize", Box::new(e)))?,
		))
}

const UPLOAD_MAX_DATA: usize = 1024 * 1024 * 32; // 32MiB, both image and JSON data

async fn process_multipart_image(
	mut payload: Multipart,
) -> Result<(Vec<u8>, String, serde_json::Value), APIError> {
	// Get the multipart data
	let mut image_data = Vec::new();
	let mut filename = String::new();
	let mut json = serde_json::Value::Null;
	let mut bytes_read = 0;
	// Iterate over incoming data
	while let Ok(Some(mut field)) = payload.try_next().await {
		// Get content disposition and then name, return error if invalid/missing
		let cont_type = field
			.content_disposition()
			.ok_or(APIError::BadRequestData)?;
		let name = cont_type.get_name().ok_or(APIError::BadRequestData)?;

		// Counter how many bytes and return err if over sized
		let mut count_bytes = |new_bytes: usize| -> Result<(), APIError> {
			bytes_read += new_bytes;
			if bytes_read >= UPLOAD_MAX_DATA {
				Err(APIError::PayloadSize)
			} else {
				Ok(())
			}
		};
		// Iterator over chunks in field
		match name {
			"image" => {
				// Read data and check that is within size limit
				while let Some(Ok(chunk)) = field.next().await {
					count_bytes(chunk.len())?;
					image_data.extend_from_slice(&chunk);
				}
				filename = sanitize_filename::sanitize(
					cont_type.get_filename().ok_or(APIError::BadRequestData)?,
				);
			}
			"data" => {
				// Temporarily store the data, we could implement a reader to avoid a memcpy but eh
				let mut data = Vec::new();
				while let Some(Ok(chunk)) = field.next().await {
					count_bytes(chunk.len())?;
					data.extend_from_slice(&chunk);
				}
				json = serde_json::from_slice(&data).map_err(|_e| APIError::BadRequestData)?;
			}
			_ => {
				// This is effectively ignored, but count the amount of bytes
				// anyway so we know we aren't being sent to much data
				while let Some(Ok(chunk)) = field.next().await {
					count_bytes(chunk.len())?;
				}
			}
		}
	}
	Ok((image_data, filename, json))
}
