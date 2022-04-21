use std::io::{BufReader, Read};

use actix_cors::Cors;
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

	// Decide what we need to do
	match settings.action {
		Action::RunServer => run_server(settings).await?,
		Action::InstallSchema => {
			println!("Installing database schema...");
			database::install_schema(settings).await
		}
		Action::DropTables => {
			println!(
				"CAUTION: Are you sure you want to drop all the tables? This will delete any data \
				 stored, image data will be unaffected (y/N)"
			);
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
			println!("Copying default images...");
			let image = include_bytes!("../res/default_pfp.png");
			std::fs::write(format!("{}/pfp/default.png", settings.storage_root), image).ok();
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
	let http_listener = std::net::TcpListener::bind(&settings.server_host)?;
	log::info!(
		"Watame Server Listening on {}",
		http_listener.local_addr().unwrap()
	);

	#[cfg(feature = "host-storage")]
	let storage_root = std::mem::take(&mut settings.storage_root);

	let server = HttpServer::new(move || {
		use actix_web::web::{delete, get, post, resource, QueryConfig};
		use pages::*;

		let cors = Cors::default()
			.allow_any_origin()
			.allow_any_method()
			.max_age(3600);
		let query_config = QueryConfig::default().error_handler(|a, b| {
			log::error!("{:?} {:?}", a, b);
			error::APIError::BadRequestData.into()
		});

		// Wrap up any data or middleware that the actix web server will use
		let app = App::new()
			.wrap(cors)
			.wrap(middleware::Logger::new("\t%a\t\"%r\"\t%s\t%b\t%Dms"))
			.wrap(auth::AuthMiddlewareFactory::new(auth::AuthDb::new(
				auth_db.clone(),
			)))
			.app_data(Data::new(db_pool.clone()))
			.app_data(Data::new(auth::AuthDb::new(auth_db.clone())))
			.app_data(Data::new(run_settings.clone()))
			.app_data(query_config);

		// Set our servers routes
		let app = app
			.service(
				resource("/post")
					.route(delete().to(post::delete_post))
					.route(get().to(post::get_post))
					.route(post().to(post::post_upload)),
			)
			.service(resource("/user").route(get().to(user::get_self)))
			.service(resource("/register").route(post().to(user::post_register)))
			.service(resource("/login").route(post().to(user::post_login)))
			.service(resource("/logout").route(delete().to(user::delete_logout)))
			.service(resource("/loggedin").route(get().to(user::get_logged_in)))
			.service(resource("/purge").route(delete().to(post::delete_purge_posts)))
			.service(resource("/tag").route(get().to(tag::get_info)))
			.service(resource("/search").route(get().to(search::get_search)))
			.service(resource("/random").route(get().to(search::get_random_post)));
		#[cfg(feature = "host-storage")]
		let app = app.service(actix_files::Files::new("/s", &storage_root));

		app
	});

	// Run the server either with HTTPS or not
	if settings.use_https {
		let config: rustls::server::ServerConfig = get_tls_config(&settings);
		server.listen_rustls(http_listener, config)?.run().await
	} else {
		server.listen(http_listener)?.run().await
	}
}

fn get_tls_config(settings: &Settings) -> rustls::server::ServerConfig {
	// Open files
	let cert_file = &mut BufReader::new(
		std::fs::File::open(&settings.cert).expect("failed to open certs file"),
	);
	let key_file = &mut BufReader::new(
		std::fs::File::open(&settings.priv_key).expect("failed to open priv key file"),
	);

	// Parse files
	let cert_chain = rustls_pemfile::certs(cert_file)
		.unwrap()
		.into_iter()
		.map(rustls::Certificate)
		.collect();
	let mut keys = rustls_pemfile::pkcs8_private_keys(key_file).unwrap();
	if keys.is_empty() {
		log::error!("couldn't find keys");
		std::process::exit(1);
	}

	// Create TLS config
	rustls::ServerConfig::builder()
		.with_safe_defaults()
		.with_no_client_auth()
		.with_single_cert(cert_chain, rustls::PrivateKey(keys.remove(0)))
		.expect("bad certs/key")
}
