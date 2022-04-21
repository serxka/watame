use pg::types::ToSql;

use crate::database::{enums::*, pg, tag::TagVector, DatabaseError};
use crate::pages::search::PostSorting;

pub type Timestamp = chrono::DateTime<chrono::offset::Utc>;

#[derive(serde::Serialize)]
pub struct PostFull {
	pub id: i64,
	pub poster: i32,
	pub tag_vector: TagVector,
	pub create_date: Timestamp,
	pub modified_date: Timestamp,
	pub description: Option<String>,
	pub rating: Rating,
	pub score: i32,
	pub views: i32,
	pub source: Option<String>,
	pub filename: String,
	pub path: String,
	pub ext: ImageExtension,
	pub size: i32,
	pub width: i32,
	pub height: i32,
	pub is_deleted: bool,
}

pub enum Post {
	Partial(i64),
	Full(PostFull),
}

impl Post {
	pub fn get_id(&self) -> i64 {
		match self {
			Self::Partial(id) => *id,
			Self::Full(post) => post.id,
		}
	}

	pub fn into_full(self) -> PostFull {
		match self {
			Self::Full(post) => post,
			_ => panic!("tried to get full post when wasn't full!"),
		}
	}

	pub fn as_full(&self) -> &PostFull {
		match self {
			Self::Full(ref post) => post,
			_ => panic!("tried to get full post when wasn't full!"),
		}
	}

	fn if_full<F: FnOnce(&mut PostFull)>(&mut self, f: F) {
		match self {
			Post::Partial(_) => {}
			Post::Full(ref mut p) => f(p),
		}
	}

	fn deserialise<'a>(row: &'a pg::row::Row) -> Self {
		// Try and get the 17th column (is_deleted) to see if a full post or partial
		match row.try_get::<usize, bool>(16) {
			Ok(_) => Post::Full(Self::deserialise_full(row)),
			Err(_) => Post::Partial(row.get(0)),
		}
	}

	pub fn deserialise_full<'a>(row: &'a pg::row::Row) -> PostFull {
		PostFull {
			id: row.get(0),
			poster: row.get(1),
			tag_vector: row.get(2),
			create_date: row.get(3),
			modified_date: row.get(4),
			description: row.get(5),
			rating: row.get(6),
			score: row.get(7),
			views: row.get(8),
			source: row.get(9),
			filename: row.get(10),
			path: row.get(11),
			ext: row.get(12),
			size: row.get(13),
			width: row.get(14),
			height: row.get(15),
			is_deleted: row.get(16),
		}
	}
}

impl Post {
	pub async fn select_post<C: pg::GenericClient>(
		client: &C,
		id: i64,
	) -> Result<Option<Self>, DatabaseError> {
		let query = "SELECT * FROM posts WHERE id=$1 AND is_deleted='false'";
		let row = client
			.query_opt(query, &[&id])
			.await
			.map_err(|e| DatabaseError::from(e))?;
		match row {
			Some(row) => Ok(Some(Self::deserialise(&row))),
			None => Ok(None),
		}
	}

	pub async fn select_can_delete<C: pg::GenericClient>(
		client: &C,
		id: i64,
		user: i32,
	) -> Result<Option<(bool, Self)>, DatabaseError> {
		// Select the post to see if it exists, also get the poster id
		let params: [&(dyn ToSql + Sync); 1] = [&id];
		let query = "SELECT * FROM posts WHERE id=$1 AND is_deleted='false'";
		let row1 = client.query_opt(query, &params);

		// Select the user and see if they have permissions
		let params: [&(dyn ToSql + Sync); 1] = [&user];
		let query = "SELECT * FROM users WHERE id=$1";
		let row2 = client.query_one(query, &params);

		// Await on these
		let (post_row, user_row) =
			futures::try_join!(row1, row2).map_err(|e| DatabaseError::from(e))?;

		match post_row {
			Some(post) => {
				let perms = user_row.get(5);
				let post = Self::deserialise(&post);
				let poster = post.as_full().poster;
				if poster == user || matches!(perms, Perms::Moderator | Perms::Admin) {
					Ok(Some((true, post)))
				} else {
					Ok(Some((false, post)))
				}
			}
			None => Ok(None),
		}
	}

	pub async fn select_is_deleted<C: pg::GenericClient>(
		client: &C,
	) -> Result<Vec<PostFull>, DatabaseError> {
		let query = "SELECT * FROM posts WHERE is_deleted='true'";
		let rows = client
			.query(query, &[])
			.await
			.map_err(|e| DatabaseError::from(e))?;
		let mut posts = Vec::new();
		for row in rows {
			posts.push(Self::deserialise_full(&row));
		}
		Ok(posts)
	}

	pub async fn select_post_random<C: pg::GenericClient>(
		client: &C,
	) -> Result<Option<Self>, DatabaseError> {
		let query = "SELECT * FROM posts ORDER BY RANDOM() LIMIT 1 WHERE is_deleted='false'";
		let row = client
			.query_opt(query, &[])
			.await
			.map_err(|e| DatabaseError::from(e))?;
		match row {
			Some(row) => Ok(Some(Self::deserialise(&row))),
			None => Ok(None),
		}
	}

	pub async fn select_fulltext_tags<C: pg::GenericClient>(
		client: &C,
		tags: &[&str],
		page: u32,
		limit: u32,
		sorting: PostSorting,
	) -> Result<Vec<PostFull>, DatabaseError> {
		// If there are no tags, then run other version
		if tags.len() == 0 {
			return Self::select_fulltext_empty(client, page, limit, sorting).await;
		}
		let (t_inc, t_exc) = ts_query_builder(tags);
		let rows = if t_exc.is_empty() {
			let query = format!(
				"SELECT * FROM posts WHERE tag_vector @@ plainto_tsquery('tag_parser', $1) AND \
				 is_deleted='false' {} OFFSET {} LIMIT {}",
				sorting.to_sql(),
				page * limit,
				limit
			);
			client
				.query(query.as_str(), &[&t_inc])
				.await
				.map_err(|e| DatabaseError::from(e))?
		} else if t_inc.is_empty() {
			let query = format!(
				"SELECT * FROM posts WHERE NOT tag_vector @@ plainto_tsquery('tag_parser', $1) \
				 AND is_deleted='false' {} OFFSET {} LIMIT {}",
				sorting.to_sql(),
				page * limit,
				limit
			);
			println!("gaming, {}", query);
			client
				.query(query.as_str(), &[&t_exc])
				.await
				.map_err(|e| DatabaseError::from(e))?
		} else {
			let query = format!(
				"SELECT * FROM posts WHERE tag_vector @@ plainto_tsquery('tag_parser', $1) AND \
				 NOT tag_vector @@ plainto_tsquery('tag_parser', $2) AND     is_deleted='false' \
				 {} OFFSET {} LIMIT {}",
				sorting.to_sql(),
				page * limit,
				limit
			);
			client
				.query(query.as_str(), &[&t_inc, &t_exc])
				.await
				.map_err(|e| DatabaseError::from(e))?
		};

		let mut posts = Vec::new();
		for row in rows {
			posts.push(Self::deserialise_full(&row));
		}
		Ok(posts)
	}

	async fn select_fulltext_empty<C: pg::GenericClient>(
		client: &C,
		page: u32,
		limit: u32,
		sorting: PostSorting,
	) -> Result<Vec<PostFull>, DatabaseError> {
		let query = format!(
			"SELECT * FROM posts WHERE is_deleted='false' {} OFFSET {} LIMIT {}",
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
			posts.push(Self::deserialise_full(&row));
		}
		Ok(posts)
	}

	pub async fn update_path<C: pg::GenericClient>(
		&mut self,
		client: &C,
		new_path: &str,
	) -> Result<(), DatabaseError> {
		let query = "UPDATE posts SET path=$1 WHERE id=$2";
		client
			.execute(query, &[&new_path, &self.get_id()])
			.await
			.map_err(|e| DatabaseError::from(e))?;
		self.if_full(|p| {
			p.path.clear();
			p.path.push_str(new_path);
		});
		Ok(())
	}

	pub async fn update_is_deleted<C: pg::GenericClient>(
		&mut self,
		client: &C,
		is_deleted: bool,
	) -> Result<(), DatabaseError> {
		let query = "UPDATE posts SET is_deleted=$1 WHERE id=$2";
		client
			.execute(query, &[&is_deleted, &self.get_id()])
			.await
			.map_err(|e| DatabaseError::from(e))?;
		self.if_full(|p| {
			p.is_deleted = is_deleted;
		});
		Ok(())
	}

	pub async fn delete_post_checked<C: pg::GenericClient>(
		&self,
		client: &C,
	) -> Result<bool, DatabaseError> {
		let query = "DELETE FROM posts WHERE id=$1 AND is_deleted = 'true'";
		let res = client
			.execute(query, &[&self.get_id()])
			.await
			.map_err(|e| DatabaseError::from(e))?;
		Ok(res != 0)
	}
}

fn ts_query_builder(tags: &[&str]) -> (String, String) {
	let mut include = String::new();
	let mut exclude = String::new();
	for tag in tags {
		if &tag[0..1] == "!" {
			exclude.push_str(&tag[1..]);
			exclude.push(',');
		} else {
			include.push_str(tag);
			include.push(',');
		}
	}
	include.pop();
	exclude.pop();
	println!("{:?} , {:?}", include, exclude);
	(include, exclude)
}

impl std::convert::From<i64> for Post {
	fn from(id: i64) -> Post {
		Post::Partial(id)
	}
}

#[derive(Debug)]
pub struct NewPost<'a> {
	pub filename: &'a str,
	pub ext: ImageExtension,
	pub path: &'a str,
	pub size: i32,
	pub dimensions: (i32, i32),
	pub rating: Rating,
	pub description: &'a str,
	pub tags: &'a [&'a str],
	pub poster: i32,
}

impl NewPost<'_> {
	pub async fn insert_into<C: pg::GenericClient>(
		&self,
		client: &C,
	) -> Result<PostFull, DatabaseError> {
		let query = "INSERT INTO posts (filename, path, ext, size, width, height, description, \
		             rating, tag_vector, poster) VALUES($1, $2, $3, $4, $5, $6, $7, $8, \
		             to_tsvector('tag_parser', $9), $10) RETURNING *";
		let tags: String = self
			.tags
			.iter()
			.flat_map(|s| s.chars().chain([',']))
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
					&self.rating,
					&tags,
					&self.poster,
				],
			)
			.await
			.map_err(|e| DatabaseError::from(e))?;
		Ok(Post::deserialise_full(&row))
	}
}
