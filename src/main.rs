use actix_files::Files;
use actix_web::{middleware::Logger, web::Data, App, HttpServer};
use log::LevelFilter;
use std::io::Read;

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
				database::drop_tables(settings).await;
			} else {
				println!("Canceled, tables not dropped");
			}
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

		App::new()
			.wrap(Logger::new("\t%a\t\"%r\"\t%s\t%b\t%Dms"))
			.app_data(Data::new(db_pool.clone()))
			.app_data(Data::new(run_settings.clone()))
			.app_data(
				QueryConfig::default().error_handler(|_, _| error::APIError::BadRequestData.into()),
			)
			.service(
				resource("/post")
					.route(delete().to(post::delete_post))
					.route(get().to(post::get_post))
					.route(post().to(post::post_upload)),
			)
			.service(resource("/search").route(get().to(search::get_search)))
			.service(Files::new("/s", &storage_root))
			// Debugging routes
			.service(resource("/upload").route(get().to(pages::upload_post_html)))
	})
	.listen(http_listener)?
	.run()
	.await
}
