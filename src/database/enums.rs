use crate::database::pg;
use crate::pages::search::PostSorting;

use pg::types::{FromSql as FromSqlDerive, ToSql as ToSqlDerive};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSqlDerive, FromSqlDerive)]
#[postgres(name = "perms")]
pub enum Perms {
	Guest,
	User,
	Moderator,
	Admin,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSqlDerive, FromSqlDerive)]
#[postgres(name = "rating")]
pub enum Rating {
	Safe,
	Sketchy,
	Explicit,
}

impl core::default::Default for Rating {
	fn default() -> Self {
		Rating::Sketchy
	}
}

#[derive(Debug, Clone, Copy, Serialize, ToSqlDerive, FromSqlDerive)]
#[postgres(name = "imgext")]
pub enum ImageExtension {
	Bmp,
	Gif,
	Jpg,
	Png,
	Tiff,
	Webp,
}

impl std::convert::From<image::ImageFormat> for ImageExtension {
	fn from(im: image::ImageFormat) -> Self {
		use image::ImageFormat;
		match im {
			ImageFormat::Bmp => ImageExtension::Bmp,
			ImageFormat::Gif => ImageExtension::Gif,
			ImageFormat::Jpeg => ImageExtension::Jpg,
			ImageFormat::Png => ImageExtension::Png,
			ImageFormat::Tiff => ImageExtension::Tiff,
			ImageFormat::WebP => ImageExtension::Webp,
			_ => panic!("unknown image format: {:?}", im),
		}
	}
}

impl PostSorting {
	pub fn to_sql(&self) -> &str {
		match self {
			PostSorting::DateAscending => "ORDER BY create_date ASC, id ASC",
			PostSorting::DateDescending => "ORDER BY create_date DESC, id DESC",
			PostSorting::VoteAscending => "ORDER BY score ASC, id ASC",
			PostSorting::VoteDescending => "ORDER BY score DESC, id DESC",
		}
	}
}
