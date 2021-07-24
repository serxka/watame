pub use deadpool_postgres::tokio_postgres as pg;
use derive_more::{Display, Error};

#[derive(Debug, Display, Error)]
pub enum DatabaseError {
	PostgresErr(pg::error::Error),
}

impl std::convert::From<pg::error::Error> for DatabaseError {
	fn from(err: pg::error::Error) -> Self {
		Self::PostgresErr(err)
	}
}
