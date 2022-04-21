pub use deadpool_postgres::tokio_postgres as pg;

use crate::database::{enums::Perms, DatabaseError};

use serde::Serialize;

#[derive(Serialize)]
pub struct User {
	pub id: i32,
	pub name: String,
	pub email: Option<String>,
	pub pass: String,
	pub picture: String,
	pub perms: Perms,
}

impl User {
	pub async fn select_id<C: pg::GenericClient>(
		client: &C,
		uid: i32,
	) -> Result<Option<User>, DatabaseError> {
		let query = "SELECT * FROM users WHERE id=$1";
		let row = client
			.query_opt(query, &[&uid])
			.await
			.map_err(|e| DatabaseError::from(e))?;
		Ok(row.as_ref().map(|row| Self::deserialise(row)))
	}

	pub async fn select_name<C: pg::GenericClient>(
		client: &C,
		name: &str,
	) -> Result<Option<User>, DatabaseError> {
		let query = "SELECT * FROM users WHERE name=$1";
		let row = client
			.query_opt(query, &[&name])
			.await
			.map_err(|e| DatabaseError::from(e))?;
		Ok(row.as_ref().map(|row| Self::deserialise(row)))
	}

	pub async fn check_existence<C: pg::GenericClient>(
		client: &C,
		name: &str,
		email: Option<&str>,
	) -> Result<bool, DatabaseError> {
		let query = "SELECT id FROM users WHERE name=$1 OR email=$2";
		let row = client
			.query_opt(query, &[&name, &email])
			.await
			.map_err(|e| DatabaseError::from(e))?;
		match row {
			Some(_) => Ok(true),
			None => Ok(false),
		}
	}

	fn deserialise<'a>(row: &'a pg::row::Row) -> Self {
		User {
			id: row.get(0),
			name: row.get(1),
			email: row.get(2),
			pass: row.get(3),
			picture: row.get(4),
			perms: row.get(5),
		}
	}
}

#[derive(Debug)]
pub struct NewUser<'a> {
	pub name: &'a str,
	pub email: Option<&'a str>,
	pub pass: &'a str,
	pub picture: Option<&'a str>,
}

impl<'a> NewUser<'a> {
	pub async fn insert_into<C: pg::GenericClient>(
		&self,
		client: &C,
	) -> Result<User, DatabaseError> {
		let query =
			"INSERT INTO users (name, email, pass, picture) VALUES($1, $2, $3, $4) RETURNING *";
		let row = client
			.query_one(
				query,
				&[
					&self.name,
					&self.email,
					&self.pass,
					&self.picture.unwrap_or("/s/pfp/default.png"),
				],
			)
			.await
			.map_err(|e| DatabaseError::from(e))?;
		Ok(User::deserialise(&row))
	}
}
