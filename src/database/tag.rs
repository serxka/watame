use byteorder::{BigEndian, ByteOrder};
use bytes::{buf::BufMut, BytesMut};
use pg::types::{FromSql, IsNull, ToSql, Type};
use std::borrow::ToOwned;

use crate::database::pg;

#[derive(Debug, serde::Serialize)]
pub struct Tags(pub Vec<String>);

impl ToSql for Tags {
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

impl<'a> FromSql<'a> for Tags {
	fn from_sql(
		_ty: &Type,
		raw: &'a [u8],
	) -> Result<Self, Box<(dyn std::error::Error + Sync + Send + 'static)>> {
		let tags = tsvector_from_sql(raw)
			.into_iter()
			.map(|s| s.to_owned())
			.collect();
		Ok(Tags(tags))
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
