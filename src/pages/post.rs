use async_std::fs;
use std::io::Cursor;
use std::path::PathBuf;

use crate::auth::Authenticated;
use crate::database::{
	enums::{Perms, Rating},
	pg,
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
	let img_path = [root, "img", &subfolder, &format!("{}-{}", id, filename)]
		.iter()
		.collect();
	// File path for the smaller thumbnail
	let tmb_path = [root, "tmb", &subfolder, &format!("{}.jpg", id)]
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
		Post::select_post::<pg::Client>(&conn, query.id).await,
		"get_post:select_id {}",
		query.id
	);

	// Check to see if we actually found a post
	match post {
		Some(x) => Ok(HttpResponse::Ok()
			.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
			.body(serde_json::to_string(x.as_full()).unwrap())),
		None => Ok(HttpResponse::NotFound()
			.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
			.body(r#"{"error":"post not found"}"#)),
	}
}

pub async fn delete_post(
	query: web::Query<IdPostQuery>,
	pool: web::Data<DbPool>,
	auth: Authenticated,
) -> Result<HttpResponse, APIError> {
	// Verify we haven't been given a negative ID
	if query.id < 0 {
		return Err(APIError::BadRequestData);
	}

	// Query database for post
	let mut conn = try500!(pool.get().await, "delete_post:db pool");
	let trans = try500!(conn.transaction().await);
	let post = try500!(
		Post::select_can_delete::<pg::Transaction<'_>>(&trans, query.id, auth.uid).await,
		"delete_post:select_id_poster {}",
		query.id
	);

	// if it exists and we are the owner we can delete it
	let res = match post {
		Some((true, mut post)) => {
			try500!(
				post.update_is_deleted::<pg::Transaction<'_>>(&trans, true)
					.await,
				"delete_post:update_is_deleted"
			);
			let post = post.into_full();
			// Also decrease our tag count
			try500!(
				Tag::update_decrease_counts::<pg::Transaction<'_>>(&trans, &post.tag_vector.0)
					.await,
				"delete_post:update_decrease_counts {:?}",
				post.tag_vector
			);
			Ok(HttpResponse::Ok()
				.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
				.body(r#"{"success":"post deleted"}"#))
		}
		Some((false, _)) => Err(APIError::Auth),
		None => Ok(HttpResponse::NotFound()
			.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
			.body(r#"{"error":"post not found"}"#)),
	};
	// Commit our changes, if any
	try500!(trans.commit().await);

	res
}

pub async fn delete_purge_posts(
	pool: web::Data<DbPool>,
	settings: web::Data<RunSettings>,
	auth: Authenticated,
) -> Result<HttpResponse, APIError> {
	if auth.perms != Perms::Admin {
		return Err(APIError::Auth);
	}

	let conn = try500!(pool.get().await, "delete_post:db pool");
	let posts = try500!(
		Post::select_is_deleted::<pg::Client>(&conn).await,
		"delete_purge_posts:select"
	);
	for post in posts {
		// Check to make sure we only delete if the image is still marked to be deleted
		if try500!(
			Post::Partial(post.id)
				.delete_post_checked::<pg::Client>(&conn)
				.await,
			"delete_post"
		) == false
		{
			continue;
		}
		// Delete the image files on disk
		let (img_path, tmb_path) =
			format_paths(&settings.storage_root, &post.path, post.id, &post.filename);
		let (img, tmb) = futures::join!(fs::remove_file(&img_path), fs::remove_file(&tmb_path),);
		try500!(img, "image delete {}", img_path.display());
		try500!(tmb, "thumb delete {}", tmb_path.display());
	}
	Ok(HttpResponse::Ok()
		.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
		.body(r#"{"success":"posts purged"}"#))
}

#[derive(serde::Deserialize)]
struct NewPostDetails {
	tags: Vec<String>,
	#[serde(default = "String::new")]
	description: String,
	#[serde(default = "Rating::default")]
	rating: Rating,
}

pub async fn post_upload(
	payload: Multipart,
	pool: web::Data<DbPool>,
	settings: web::Data<RunSettings>,
	auth: Authenticated,
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
	let details: NewPostDetails = serde_json::from_value(json)
		.ok()
		.ok_or(APIError::BadRequestData)?;
	let ta = details.tags;

	// Check that tags are valid and add them to an array
	let mut tags = Vec::with_capacity(ta.len());
	for i in 0..ta.len() {
		if ta[i].chars().any(|c| matches!(c, '+' | '!')) {
			return Err(APIError::BadTags);
		}
		tags.push(ta[i].trim());
	}

	// Fill in the details for our now post
	let new_post = NewPost {
		filename: &filename,
		ext: image_type.into(),
		path: "00",
		size: file_size as i32,
		dimensions: (dimensions.0 as i32, dimensions.1 as i32),
		description: &details.description,
		rating: details.rating,
		tags: &tags,
		poster: auth.uid,
	};

	let mut conn = try500!(pool.get().await, "post_upload:db pool");
	let trans = try500!(conn.transaction().await);
	let post = try500!(
		new_post.insert_into::<pg::Transaction<'_>>(&trans).await,
		"post_upload:insert_into {:?}",
		new_post
	);

	// Also insert/update our tags
	let _ = try500!(
		Tag::update_tag_count::<pg::Transaction<'_>>(&trans, &tags).await,
		"post_upload:update_tag_count {:?}",
		tags
	);

	let subfolder = image_path(post.id);
	try500!(
		Post::Partial(post.id)
			.update_path::<pg::Transaction<'_>>(&trans, &subfolder)
			.await,
		"post_upload:update_path"
	);

	let (img_path, tmb_path) =
		format_paths(&settings.storage_root, &subfolder, post.id, &post.filename);

	// Async fs write the main image as it's already encoded
	let img = fs::write(&img_path, &image_data);
	// We have to first encoder the thumbnail as a Jpeg before we can write it
	let mut tmb_data = Cursor::new(Vec::new());
	try500!(
		thumbnail.write_to(&mut tmb_data, image::ImageOutputFormat::Jpeg(90)),
		"jpeg encode"
	);
	let tmb_data = tmb_data.into_inner();
	let tmb = fs::write(&tmb_path, &tmb_data);

	// Take these two futures and wait on them
	let (img, tmb) = futures::join!(img, tmb);
	try500!(img, "image write {}", img_path.display());
	try500!(tmb, "thumb write {}", tmb_path.display());

	// Commit our transaction
	try500!(trans.commit().await);

	Ok(HttpResponse::Ok()
		.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
		.body(serde_json::to_string(&post).unwrap()))
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
		let cont_type = field.content_disposition().clone();
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
				// Temporarily store the data, we could implement a reader to avoid a memcpy but
				// eh
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

	let dim = image::GenericImageView::dimensions(image);
	let sub = if dim.0 < dim.1 {
		imageops::crop(image, 0, (dim.1 - dim.0) / 2, dim.0, dim.0)
	} else if dim.0 >= dim.1 {
		imageops::crop(image, (dim.0 - dim.1) / 2, 0, dim.1, dim.1)
	} else {
		unreachable!()
	};
	DynamicImage::ImageRgba8(imageops::thumbnail(&sub, THUMB_SIZE, THUMB_SIZE))
	// unimplemented!()
	// Alternative thumbnail creation
	// let thumbnail = image.thumbnail(320, 320);
}
