use std::io::Read;

use actix_cors::Cors;
use actix_files::Files;
use actix_web::{middleware, web::Data, App, HttpServer};
use log::LevelFilter;

mod auth;
mod database;
mod error;
mod pages;
mod settings;

use settings::{Action, RunSettings, Settings};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
	simple_logger::SimpleLogger::new()
		.with_level(LevelFilter::Info)
		.init()
		.unwrap();
	dotenv::dotenv().ok();

	let settings = Settings::parse();

	// Decide what are need to do
	match settings.action {
		Action::RunServer => run_server(settings).await?,
		Action::InstallSchema => {
			println!("Installing database schema...");
			database::install_schema(settings).await
		}
		Action::DropTables => {
			println!("CAUTION: Are you sure you want to drop all the tables? This will delete any data stored, image data will be unaffected (y/N)");
			let mut answer = [0];
			std::io::stdin()
				.read_exact(&mut answer)
				.expect("failed to read from stdin");
			let answer = answer[0] as char;
			if answer == 'Y' || answer == 'y' {
				println!("Dropping tables...");
				auth::AuthDbCreator::clear_sessions(&settings.redis_uri).await;
				database::drop_tables(settings).await;
			} else {
				println!("Cancelled, tables not dropped");
			}
		}
		Action::ClearSessions => {
			println!("Clearing User Sessions...");
			auth::AuthDbCreator::clear_sessions(&settings.redis_uri).await;
		}
		Action::CreateFolders => {
			let image_dirs = |root| {
				for i in 0..256 {
					let path = format!("{}/{:02x}", root, i);
					match std::fs::create_dir_all(&path) {
						Ok(_) => {}
						Err(e) => {
							log::error!("({}): Failed to create dir: {}", e, path);
							std::process::exit(1);
						}
					}
				}
			};
			println!("Creating folders...");
			image_dirs(format!("{}/img", settings.storage_root));
			image_dirs(format!("{}/tmb", settings.storage_root));
			image_dirs(format!("{}/pfp", settings.storage_root));
		}
	}
	Ok(())
}

async fn run_server(mut settings: Settings) -> std::io::Result<()> {
	// Connect to the database and create a connection pool
	let db_pool = database::establish_pool(&mut settings);
	let auth_db = auth::AuthDbCreator::new(&settings.redis_uri).await;
	// Settings that handlers can access
	let run_settings = RunSettings::from(&settings);
	// Create a listener so we can log what port we are operating on
	let http_listener = std::net::TcpListener::bind(settings.server_host)?;
	log::info!(
		"Watame Server Listening on {}",
		http_listener.local_addr().unwrap()
	);

	let storage_root = std::mem::take(&mut settings.storage_root);
	HttpServer::new(move || {
		use actix_web::web::{delete, get, post, resource, QueryConfig};
		use pages::*;

		let cors = Cors::default()
			.allow_any_origin()
			.allow_any_method()
			.max_age(3600);

		App::new()
			.wrap(cors)
			.wrap(middleware::Logger::new("\t%a\t\"%r\"\t%s\t%b\t%Dms"))
			.wrap(auth::AuthMiddlewareFactory::new(auth::AuthDb::new(
				auth_db.clone(),
			)))
			.app_data(Data::new(db_pool.clone()))
			.app_data(Data::new(auth::AuthDb::new(auth_db.clone())))
			.app_data(Data::new(run_settings.clone()))
			.app_data(QueryConfig::default().error_handler(|a, b| {
				log::error!("{:?} {:?}", a, b);
				error::APIError::BadRequestData.into()
			}))
			.service(
				resource("/post")
					.route(delete().to(post::delete_post))
					.route(get().to(post::get_post))
					.route(post().to(post::post_upload)),
			)
			.service(resource("/register").route(post().to(user::post_register)))
			.service(resource("/login").route(post().to(user::post_login)))
			.service(resource("/logout").route(delete().to(user::delete_logout)))
			.service(resource("/purge").route(delete().to(post::delete_purge_posts)))
			.service(resource("/tag").route(get().to(tag::get_info)))
			.service(resource("/search").route(get().to(search::get_search)))
			.service(resource("/random").route(get().to(search::get_random_post)))
			// Debugging routes
			.service(Files::new("/s", &storage_root))
			.service(resource("/upload").route(get().to(pages::upload_post_html)))
	})
	.listen(http_listener)?
	.run()
	.await
}
