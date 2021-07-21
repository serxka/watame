pub use deadpool_postgres::tokio_postgres as pg;

use crate::database::DatabaseError;
use crate::pages::search::PostSorting;

#[derive(Debug, serde::Serialize)]
pub enum ImageExtension {
	Bmp,
	Gif,
	Jpeg,
	Png,
	Tiff,
	Webp,
}

impl ImageExtension {
	pub fn as_str(&self) -> &'static str {
		match self {
			ImageExtension::Bmp => "bmp",
			ImageExtension::Gif => "gif",
			ImageExtension::Jpeg => "jpeg",
			ImageExtension::Png => "png",
			ImageExtension::Tiff => "tiff",
			ImageExtension::Webp => "webp",
		}
	}

	pub fn from_str(ext: &str) -> Option<ImageExtension> {
		match ext {
			"bmp" => Some(ImageExtension::Bmp),
			"gif" => Some(ImageExtension::Gif),
			"jpeg" => Some(ImageExtension::Jpeg),
			"png" => Some(ImageExtension::Png),
			"tiff" => Some(ImageExtension::Tiff),
			"webp" => Some(ImageExtension::Webp),
			_ => None,
		}
	}
}

impl std::convert::From<image::ImageFormat> for ImageExtension {
	fn from(im: image::ImageFormat) -> Self {
		use image::ImageFormat;
		match im {
			ImageFormat::Bmp => ImageExtension::Bmp,
			ImageFormat::Gif => ImageExtension::Gif,
			ImageFormat::Jpeg => ImageExtension::Jpeg,
			ImageFormat::Png => ImageExtension::Png,
			ImageFormat::Tiff => ImageExtension::Tiff,
			ImageFormat::WebP => ImageExtension::Webp,
			_ => panic!("unknown image format: {:?}", im),
		}
	}
}

impl std::fmt::Display for ImageExtension {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

impl pg::types::ToSql for ImageExtension {
	fn to_sql(
		&self,
		ty: &pg::types::Type,
		w: &mut bytes::BytesMut,
	) -> Result<pg::types::IsNull, Box<(dyn std::error::Error + Sync + Send + 'static)>> {
		<&str as pg::types::ToSql>::to_sql(&self.as_str(), ty, w)
	}
	fn accepts(ty: &pg::types::Type) -> bool {
		use pg::types::Type;
		match *ty {
			Type::ANY => true,
			_ => true,
		}
	}

	pg::types::to_sql_checked!();
}

impl<'a> pg::types::FromSql<'a> for ImageExtension {
	fn from_sql(
		ty: &pg::types::Type,
		raw: &'a [u8],
	) -> Result<ImageExtension, Box<(dyn std::error::Error + Sync + Send + 'static)>> {
		let s = <&str as pg::types::FromSql>::from_sql(ty, raw)?;
		ImageExtension::from_str(s).ok_or(Box::new(DatabaseError::UnknownEnum))
	}
	fn accepts(ty: &pg::types::Type) -> bool {
		use pg::types::Type;
		match *ty {
			Type::VARCHAR | Type::TEXT | Type::BPCHAR | Type::ANYENUM | Type::UNKNOWN => true,
			_ => false,
		}
	}
}

impl PostSorting {
	pub fn to_sql(&self) -> &str {
		match self {
			PostSorting::DateAscending => "ORDER BY upload_date ASC, id ASC",
			PostSorting::DateDescending => "ORDER BY upload_date DESC, id DESC",
			PostSorting::VoteAscending => "ORDER BY score ASC, id ASC",
			PostSorting::VoteDescending => "ORDER BY score DESC, id DESC",
		}
	}
}

#[derive(Debug, serde::Serialize)]
pub struct Post {
	pub id: i64,
	pub upload_date: chrono::DateTime<chrono::offset::Utc>,
	pub filename: String,
	pub path: String,
	pub ext: ImageExtension,
	pub size: i32,
	pub width: i32,
	pub height: i32,
	pub description: String,
	pub tags: Vec<String>,
	pub score: i32,
	pub poster: i64,
}

#[derive(Debug)]
pub struct NewPost<'a> {
	pub filename: &'a str,
	pub ext: ImageExtension,
	pub path: &'a str,
	pub size: i32,
	pub dimensions: (i32, i32),
	pub description: &'a str,
	pub tags: &'a [&'a str],
	pub poster: i64,
}

impl Post {
	pub async fn select_post(client: &pg::Client, id: i64) -> Result<Option<Self>, DatabaseError> {
		let query = "SELECT * FROM posts WHERE id=$1";
		let row = client
			.query_opt(query, &[&id])
			.await
			.map_err(|e| DatabaseError::from(e))?;
		match row {
			Some(row) => Ok(Some(Self::serialise(&row))),
			None => Ok(None),
		}
	}

	pub async fn select_id_poster(
		client: &pg::Client,
		id: i64,
	) -> Result<Option<Self>, DatabaseError> {
		let query = "SELECT * FROM posts WHERE id=$1";
		let row = client
			.query_opt(query, &[&id])
			.await
			.map_err(|e| DatabaseError::from(e))?;
		match row {
			Some(row) => Ok(Some(Self::serialise(&row))),
			None => Ok(None),
		}
	}

	pub async fn select_tags(
		client: &pg::Client,
		tags: &[&str],
		page: u32,
		limit: u32,
		sorting: PostSorting,
	) -> Result<Vec<Self>, DatabaseError> {
		// If there are no tags, then run other version
		if tags.len() == 0 {
			return Self::select_tags_empty(client, page, limit, sorting).await;
		}
		let query = format!(
			"SELECT * FROM posts WHERE tags @@ $1 {} OFFSET {} LIMIT {}",
			sorting.to_sql(),
			page * limit,
			limit
		);
		let tags: String = tags.iter().flat_map(|s| s.chars().chain([' '])).collect();
		let rows = client
			.query(query.as_str(), &[&tags])
			.await
			.map_err(|e| DatabaseError::from(e))?;
		let mut posts = Vec::new();
		for row in rows {
			posts.push(Self::serialise(&row));
		}
		Ok(posts)
	}

	async fn select_tags_empty(
		client: &pg::Client,
		page: u32,
		limit: u32,
		sorting: PostSorting,
	) -> Result<Vec<Self>, DatabaseError> {
		let query = format!(
			"SELECT * FROM posts {} OFFSET {} LIMIT {}",
			sorting.to_sql(),
			page * limit,
			limit
		);
		let rows = client
			.query(query.as_str(), &[])
			.await
			.map_err(|e| DatabaseError::from(e))?;
		let mut posts = Vec::new();
		for row in rows {
			posts.push(Self::serialise(&row));
		}
		Ok(posts)
	}

	pub async fn update_path(
		&mut self,
		client: &pg::Client,
		new_path: &str,
	) -> Result<(), DatabaseError> {
		debug_assert!(new_path.len() == 2);
		let query = "UPDATE posts SET path=$1 WHERE id=$2";
		client
			.execute(query, &[&new_path, &self.id])
			.await
			.map_err(|e| DatabaseError::from(e))?;
		self.path.clear();
		self.path.push_str(new_path);
		Ok(())
	}

	pub async fn delete_post(client: &pg::Client, id: i64) -> Result<(), DatabaseError> {
		let query = "DELETE FROM posts WHERE id=$1";
		client
			.execute(query, &[&id])
			.await
			.map_err(|e| DatabaseError::from(e))?;
		Ok(())
	}

	fn serialise<'a>(row: &'a pg::row::Row) -> Self {
		Post {
			id: row.get(0),
			upload_date: row.get::<usize, chrono::DateTime<chrono::offset::Utc>>(1),
			filename: row.get(2),
			path: row.get(3),
			ext: row.get(4),
			size: row.get(5),
			width: row.get(6),
			height: row.get(7),
			description: row.get(8),
			tags: row
				.get::<usize, &'a str>(9)
				.trim()
				.split(' ')
				.map(|s| s.to_owned())
				.collect(),
			score: row.get(10),
			poster: row.get(11),
		}
	}
}

impl NewPost<'_> {
	pub async fn insert_into(&self, client: &pg::Client) -> Result<Post, DatabaseError> {
		let query = "INSERT INTO posts (filename, path, ext, size, width, height, description, tags, poster) VALUES\
		             ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING *";
		let tags: String = self
			.tags
			.iter()
			.flat_map(|s| s.chars().chain([' ']))
			.collect();

		let row = client
			.query_one(
				query,
				&[
					&self.filename,
					&self.path,
					&self.ext,
					&self.size,
					&self.dimensions.0,
					&self.dimensions.1,
					&self.description,
					&tags,
					&self.poster,
				],
			)
			.await
			.map_err(|e| DatabaseError::from(e))?;
		Ok(Post::serialise(&row))
	}
}
