use async_std::fs;
use std::path::PathBuf;

use crate::database::{
	post::{NewPost, Post},
	tag::Tag,
	Pool as DbPool,
};

use crate::settings::RunSettings;
use crate::{error::APIError, try500};

use actix_multipart::Multipart;
use actix_web::{http::header, web, HttpResponse};
use futures::{StreamExt, TryStreamExt};

fn image_path(id: i64) -> String {
	format!("{:02x}", id >> 16)
}

fn format_paths(root: &str, subfolder: &str, id: i64, filename: &str) -> (PathBuf, PathBuf) {
	// File path for the primary image
	let img_path: PathBuf = [root, "img", &subfolder, &format!("{}-{}", id, filename)]
		.iter()
		.collect();
	// File path for the smaller thumbnail
	let tmb_path: PathBuf = [root, "tmb", &subfolder, &format!("{}-thumbnail.jpg", id)]
		.iter()
		.collect();

	(img_path, tmb_path)
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
	let conn = try500!(pool.get().await, "get_post:db pool");
	let post = try500!(
		Post::select_post(&conn, query.id).await,
		"get_post:select_id {}",
		query.id
	);

	// Check to see if we actually found a post
	match post {
		Some(x) => Ok(HttpResponse::Ok()
			.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
			.body(try500!(
				serde_json::to_string(x.as_full()),
				"get_post:json serialize"
			))),
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
	let conn = try500!(pool.get().await, "delete_post:db pool");
	let post = try500!(
		Post::select_can_delete(&conn, query.id, 0).await,
		"delete_post:select_id_poster {}",
		query.id
	);

	// if it exists and we are the owner we can delete it
	match post {
		Some((can_delete, mut post)) => {
			if !can_delete {
				return Err(APIError::Auth);
			}
			try500!(
				post.update_is_deleted(&conn, true).await,
				"delete_post:update_is_deleted"
			);
			let post = post.into_full();
			// Also decrease our tag count
			try500!(
				Tag::update_decrease_counts(&conn, &post.tag_vector.0).await,
				"delete_post:update_decrease_counts {:?}",
				post.tag_vector
			);
			Ok(HttpResponse::Ok()
				.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
				.body(r#"{"success":"post deleted"}"#))
		}
		None => Ok(HttpResponse::NotFound()
			.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
			.body(r#"{"error":"post not found"}"#)),
	}
}

pub async fn delete_purge_posts(
	pool: web::Data<DbPool>,
	settings: web::Data<RunSettings>,
) -> Result<HttpResponse, APIError> {
	let conn = try500!(pool.get().await, "delete_post:db pool");
	let posts = try500!(
		Post::select_is_deleted(&conn).await,
		"delete_purge_posts:select"
	);
	for post in posts {
		let (img_path, tmb_path) =
			format_paths(&settings.storage_root, &post.path, post.id, &post.filename);
		let post = Post::Partial(post.id);
		let (img, tmb, pst) = futures::join!(
			fs::remove_file(&img_path),
			fs::remove_file(&tmb_path),
			post.delete_post(&conn)
		);
		try500!(img, "image delete {}", img_path.display());
		try500!(tmb, "thumb delete {}", tmb_path.display());
		try500!(pst, "delete_post");
	}
	Ok(HttpResponse::Ok()
		.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
		.body(r#"{"success":"posts purged"}"#))
}

pub async fn post_upload(
	payload: Multipart,
	pool: web::Data<DbPool>,
	settings: web::Data<RunSettings>,
) -> Result<HttpResponse, APIError> {
	let (image_data, filename, json) =
		process_multipart_image(payload, settings.max_payload).await?;

	// Load image into memory for thumbnail/info/hashing
	let image_type = image::guess_format(&image_data).map_err(|_| APIError::MimeType)?;
	let mut image = image::load_from_memory_with_format(&image_data, image_type)
		.map_err(|_| APIError::BadRequestData)?;

	// Image metadata
	let dimensions = image::GenericImageView::dimensions(&image);
	let file_size = image_data.len() as u32;

	// Generate thumb
	let thumbnail = create_thumbnail(&mut image);

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
		let s = tag.as_str().ok_or(APIError::BadRequestData)?;
		if s.chars().any(|c| matches!(c, '+' | '!')) {
			return Err(APIError::BadTags);
		}
		tags.push(s.trim());
	}

	let new_post = NewPost {
		filename: &filename,
		ext: image_type.into(),
		path: "00",
		size: file_size as i32,
		dimensions: (dimensions.0 as i32, dimensions.1 as i32),
		description: description,
		tags: &tags,
		poster: 0,
	};

	let conn = try500!(pool.get().await, "delete_post:db pool");
	let post = try500!(
		new_post.insert_into(&conn).await,
		"post_upload:insert_into {:?}",
		new_post
	);

	// Also insert/update our tags
	let _ = try500!(
		Tag::update_tag_count(&conn, &tags).await,
		"post_upload:update_tag_count {:?}",
		tags
	);

	let subfolder = image_path(post.id);
	try500!(
		Post::Partial(post.id).update_path(&conn, &subfolder).await,
		"post_upload:update_path"
	);

	let (img_path, tmb_path) =
		format_paths(&settings.storage_root, &subfolder, post.id, &post.filename);

	// Async fs write the main image as it's already encoded
	let img = fs::write(&img_path, &image_data);
	// We have to first encoder the thumbnail as a Jpeg before we can write it
	let mut tmb_data = Vec::new();
	try500!(
		thumbnail.write_to(&mut tmb_data, image::ImageOutputFormat::Jpeg(90)),
		"jpeg encode"
	);
	let tmb = fs::write(&tmb_path, &tmb_data);

	// Take these two futures and wait on them
	let (img, tmb) = futures::join!(img, tmb);
	try500!(img, "image write {}", img_path.display());
	try500!(tmb, "thumb write {}", tmb_path.display());

	Ok(HttpResponse::Ok()
		.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
		.body(try500!(
			serde_json::to_string(&post),
			"upload_post:json serialize"
		)))
}

async fn process_multipart_image(
	mut payload: Multipart,
	maximum_size: usize,
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
			if bytes_read >= maximum_size * 1024 {
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

fn create_thumbnail(image: &mut image::DynamicImage) -> image::DynamicImage {
	use image::{imageops, DynamicImage};
	const THUMB_SIZE: u32 = 320;

	let dimensions = image::GenericImageView::dimensions(image);
	let sub = if dimensions.0 < dimensions.1 {
		imageops::crop(image, 0, dimensions.0 / 4, dimensions.0, dimensions.0)
	} else if dimensions.0 >= dimensions.1 {
		imageops::crop(image, dimensions.1 / 4, 0, dimensions.1, dimensions.1)
	} else {
		unreachable!()
	};
	DynamicImage::ImageRgba8(imageops::thumbnail(&sub, THUMB_SIZE, THUMB_SIZE))
	// Alternative thumbnail creation
	// let thumbnail = image.thumbnail(320, 320);
}
