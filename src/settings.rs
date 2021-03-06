use structopt::StructOpt;

pub enum Action {
	ClearSessions,
	CreateFolders,
	DropTables,
	InstallSchema,
	RunServer,
}

impl std::default::Default for Action {
	fn default() -> Action {
		Action::RunServer
	}
}

impl std::str::FromStr for Action {
	type Err = &'static str;
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let res = match s {
			"clear-sessions" => Action::ClearSessions,
			"create-folders" => Action::CreateFolders,
			"drop-tables" => Action::DropTables,
			"install-schema" => Action::InstallSchema,
			"run" => Action::RunServer,
			_ => return Err("unknown action"),
		};
		Ok(res)
	}
}

#[derive(StructOpt)]
struct CliOptions {
	#[structopt(long = "action", default_value = "run")]
	action: Action,
}

pub struct Settings {
	pub server_host: String,
	pub database_host: String,
	pub database_port: u16,
	pub database_credentials: (String, String),
	pub database_name: String,
	pub storage_root: String,
	pub redis_uri: String,
	/// Max payload of multipart structures in KiB
	pub max_payload: usize,
	pub use_https: bool,
	pub priv_key: String,
	pub cert: String,

	pub action: Action,
}

impl std::default::Default for Settings {
	fn default() -> Settings {
		Settings {
			server_host: "127.0.0.1:8080".to_owned(),
			database_host: "127.0.0.1".to_owned(),
			database_port: 5432,
			database_credentials: ("postgres".to_owned(), "password".to_owned()),
			database_name: "watame".to_owned(),
			storage_root: "./storage/".to_owned(),
			redis_uri: "redis://127.0.0.1:6379".to_owned(),
			max_payload: 1024 * 64, // 64MiB
			use_https: false,
			priv_key: "key.pem".to_owned(),
			cert: "cert.pem".to_owned(),
			action: Action::default(),
		}
	}
}

impl Settings {
	pub fn parse() -> Settings {
		let mut settings = Self::default();
		if let Ok(v) = std::env::var("WATAME_HOST") {
			settings.server_host = v;
		}
		if let Ok(v) = std::env::var("WATAME_DB_HOST") {
			settings.database_host = v;
		}
		if let Ok(v) = std::env::var("WATAME_DB_PORT") {
			match v.parse() {
				Ok(v) => settings.database_port = v,
				Err(_) => log::warn!("invalid database port number"),
			}
		}
		if let Ok(v) = std::env::var("WATAME_DB_USER") {
			settings.database_credentials.0 = v;
		}
		if let Ok(v) = std::env::var("WATAME_DB_PASS") {
			settings.database_credentials.1 = v;
		}
		if let Ok(v) = std::env::var("WATAME_DB_NAME") {
			settings.database_name = v;
		}
		if let Ok(v) = std::env::var("WATAME_REDIS_URI") {
			settings.redis_uri = v;
		}
		if let Ok(v) = std::env::var("WATAME_STORAGE_ROOT") {
			settings.storage_root = v;
		}
		if let Ok(v) = std::env::var("WATAME_MAX_PAYLOAD") {
			match v.parse() {
				Ok(v) => settings.max_payload = v,
				Err(_) => log::warn!("invalid database address format: '{}'", v),
			}
		}
		if let Ok(v) = std::env::var("WATAME_USE_HTTPS") {
			match v.parse() {
				Ok(v) => settings.use_https = v,
				Err(_) => {
					log::warn!("unknown value for WATAME_USE_HTTPS, must be 'true' or 'false'")
				}
			}
		}
		if let Ok(v) = std::env::var("WATAME_PRIV_KEY") {
			settings.priv_key = v;
		}
		if let Ok(v) = std::env::var("WATAME_CERT") {
			settings.cert = v;
		}

		settings.merge_cli_opts(CliOptions::from_args());

		settings
	}

	fn merge_cli_opts(&mut self, opts: CliOptions) {
		self.action = opts.action;
	}
}

#[derive(Clone)]
pub struct RunSettings {
	pub storage_root: String,
	pub max_payload: usize,
}

impl RunSettings {
	pub fn from(settings: &Settings) -> Self {
		Self {
			storage_root: settings.storage_root.clone(),
			max_payload: settings.max_payload,
		}
	}
}
