pub mod enums;
pub mod error;
pub mod post;
pub mod tag;
pub mod user;

pub use deadpool_postgres::tokio_postgres as pg;
pub use deadpool_postgres::{Pool, Runtime};

pub use error::DatabaseError;

pub fn establish_pool(settings: &mut crate::settings::Settings) -> Pool {
	let mut cfg = deadpool_postgres::Config::new();
	cfg.dbname = Some(std::mem::take(&mut settings.database_name));
	cfg.host = Some(settings.database_host.clone());
	cfg.password = Some(std::mem::take(&mut settings.database_credentials.1));
	cfg.port = Some(settings.database_port);
	cfg.user = Some(std::mem::take(&mut settings.database_credentials.0));

	let pool = cfg
		.create_pool(Some(Runtime::Tokio1), pg::NoTls)
		.expect("failed to create database pool");
	pool
}

pub async fn install_schema(mut settings: crate::settings::Settings) {
	let pool = establish_pool(&mut settings);
	let db = pool
		.get()
		.await
		.expect("failed to get connection from pool");

	let scripts = [
		"CREATE EXTENSION IF NOT EXISTS tag_parser;",
		include_str!("../../res/sql/create_users.sql"),
		include_str!("../../res/sql/create_tags.sql"),
		include_str!("../../res/sql/create_posts.sql"),
	];

	for script in scripts {
		db.batch_execute(script)
			.await
			.expect("failed to create table");
	}
}

pub async fn drop_tables(mut settings: crate::settings::Settings) {
	let pool = establish_pool(&mut settings);
	let db = pool
		.get()
		.await
		.expect("failed to get connection from pool");

	db.batch_execute(include_str!("../../res/sql/drop_all.sql"))
		.await
		.expect("failed to drop tables");
}
