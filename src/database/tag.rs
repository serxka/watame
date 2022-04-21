use byteorder::{BigEndian, ByteOrder};
use bytes::{buf::BufMut, BytesMut};
use pg::types::{FromSql, IsNull, ToSql, Type};
use std::borrow::ToOwned;

use crate::database::{pg, DatabaseError};

#[derive(serde::Serialize)]
pub struct Tag {
	id: i64,
	name: String,
	count: i64,
	ty: i16,
}

impl Tag {
	fn deserialise<'a>(row: &'a pg::row::Row) -> Self {
		Tag {
			id: row.get(0),
			name: row.get(1),
			count: row.get(2),
			ty: row.get(3),
		}
	}
}

impl Tag {
	pub async fn select_tag_name<C: pg::GenericClient>(
		client: &C,
		name: &str,
	) -> Result<Option<Tag>, DatabaseError> {
		let query = "SELECT * FROM tags WHERE name = $1";
		let row = client
			.query_opt(query, &[&name])
			.await
			.map_err(|e| DatabaseError::from(e))?;
		match row {
			Some(row) => Ok(Some(Tag::deserialise(&row))),
			None => Ok(None),
		}
	}

	#[allow(dead_code)]
	pub async fn insert_empty<C: pg::GenericClient>(
		client: &C,
		tag: &str,
		ty: i16,
	) -> Result<(), DatabaseError> {
		let query = "INSERT INTO tags (name, type) VALUES ($1, $2)";
		client
			.execute(query, &[&tag, &ty])
			.await
			.map_err(|e| DatabaseError::from(e))?;
		Ok(())
	}

	pub async fn update_tag_count<C: pg::GenericClient>(
		client: &C,
		tags: &[&str],
	) -> Result<u64, DatabaseError> {
		let statement = client
			.prepare_typed(
				"INSERT INTO tags (name, count) VALUES ($1, 1) ON CONFLICT (name) DO UPDATE SET \
				 count = tags.count+1",
				&[Type::TEXT],
			)
			.await
			.map_err(|e| DatabaseError::from(e))?;
		let mut futures = Vec::with_capacity(tags.len());
		for tag in tags {
			let stmt = &statement;
			let fut = async move { client.execute(stmt, &[&tag]).await };
			futures.push(fut);
		}
		let mut modified = 0;
		for rows in futures::future::join_all(futures).await {
			modified += rows.map_err(|e| DatabaseError::from(e))?;
		}
		Ok(modified)
	}

	pub async fn update_decrease_counts<C: pg::GenericClient>(
		client: &C,
		tags: &[String],
	) -> Result<u64, DatabaseError> {
		let query = "UPDATE tags SET count = count-1 WHERE name = ANY($1)";
		client
			.execute(query, &[&tags])
			.await
			.map_err(|e| DatabaseError::from(e))
	}
}

/// A struct for representing and deserialising tags on posts
#[derive(Debug, serde::Serialize)]
pub struct TagVector(pub Vec<String>);

impl ToSql for TagVector {
	fn to_sql(
		&self,
		_ty: &Type,
		w: &mut BytesMut,
	) -> Result<IsNull, Box<(dyn std::error::Error + Sync + Send + 'static)>> {
		for tag in &self.0 {
			w.put_slice(tag.as_bytes())
		}
		Ok(IsNull::No)
	}
	fn accepts(ty: &Type) -> bool {
		matches!(*ty, Type::TS_VECTOR | Type::TSQUERY)
	}
	pg::types::to_sql_checked!();
}

impl<'a> FromSql<'a> for TagVector {
	fn from_sql(
		_ty: &Type,
		raw: &'a [u8],
	) -> Result<Self, Box<(dyn std::error::Error + Sync + Send + 'static)>> {
		let tags = tsvector_from_sql(raw)
			.into_iter()
			.map(|s| s.to_owned())
			.collect();
		Ok(TagVector(tags))
	}
	fn accepts(ty: &Type) -> bool {
		matches!(*ty, Type::TS_VECTOR | Type::TSQUERY)
	}
}

fn tsvector_from_sql<'a>(raw: &'a [u8]) -> Vec<&'a str> {
	let expected_tags = BigEndian::read_u32(raw);
	let mut tags = Vec::with_capacity(expected_tags as usize);
	let mut i = 4;

	// Read until we have one past all our lexemes
	while tags.len() != expected_tags as usize {
		// Read our string until NUL
		let mut j = i;
		while raw[j] != 0 {
			j += 1;
		}
		let s = std::str::from_utf8(&raw[i..j])
			.expect("tsvector lexeme contained invalid utf8 characters!");
		// Increment past null
		i = j + 1;

		// Read a u16 so we know how many lexeme positions we will have
		let expected_positions = BigEndian::read_u16(&raw[i..]) as usize;
		// Advance by our counter and over all the lexeme positions
		i += 2 + expected_positions * 2;
		tags.push(s);
	}

	tags
}
