use crate::auth::{AuthDb, Authenticated, MaybeAuthenticated};
use crate::database::{
	enums::Perms,
	pg,
	user::{NewUser, User},
	Pool as DbPool,
};
use crate::{error::APIError, try500};

use actix_web::{http::header, web, HttpRequest, HttpResponse};
use argon2::{self, Config};
use rand::Rng;
use serde::Serialize;

#[derive(Serialize)]
pub struct UserAPI {
	pub id: i32,
	pub username: String,
	pub email: Option<String>,
	pub picture: String,
	pub perms: Perms,
}

impl core::convert::From<User> for UserAPI {
	fn from(u: User) -> UserAPI {
		UserAPI {
			id: u.id,
			username: u.name,
			email: u.email,
			picture: u.picture,
			perms: u.perms,
		}
	}
}

#[derive(serde::Deserialize)]
pub struct RegisterUserQuery {
	user: String,
	pass: String,
	email: String,
}

pub async fn post_register(
	query: web::Json<RegisterUserQuery>,
	pool: web::Data<DbPool>,
) -> Result<HttpResponse, APIError> {
	// Check that none of fields are reasonable sizes
	if query.user.len() <= 3 || query.pass.len() < 8 {
		return Err(APIError::BadRequestData);
	}
	// Check that email looks valid
	/* if email is valid {
		// this will be of concern later, for testing is fine
	} */
	// Check that the username or email haven't been used before
	let mut conn = try500!(pool.get().await, "post_register:db pool");
	let trans = try500!(conn.transaction().await);
	if try500!(
		User::check_existence::<pg::Transaction<'_>>(&trans, &query.user, Some(&query.email)).await,
		"post_register:check_existence"
	) {
		return Err(APIError::UserExists);
	}

	let config = Config::default();
	let salt = rand::thread_rng().gen::<[u8; 16]>(); // yell at me later
	let hash = argon2::hash_encoded(&query.pass.as_bytes(), &salt, &config).unwrap();

	let new_user = NewUser {
		name: &query.user,
		email: Some(&query.email),
		pass: &hash,
		picture: None,
	};

	let user = try500!(
		new_user.insert_into::<pg::Transaction<'_>>(&trans).await,
		"post_register:insert_into {:?}",
		new_user
	);
	let user: UserAPI = user.into();

	// Commit our transaction
	try500!(trans.commit().await);

	Ok(HttpResponse::Ok()
		.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
		.body(serde_json::to_vec(&user).unwrap()))
}

#[derive(serde::Deserialize)]
pub struct LoginUserQuery {
	user: String,
	pass: String,
}

pub async fn post_login(
	pool: web::Data<DbPool>,
	auth_db: web::Data<AuthDb>,
	query: web::Json<LoginUserQuery>,
) -> Result<HttpResponse, APIError> {
	// Attempt to get our user from the database
	let conn = try500!(pool.get().await, "post_login:db pool");
	let user = try500!(
		User::select_name::<pg::Client>(&conn, &query.user).await,
		"post_login:select_name {:?}",
		query.user
	);
	// Check to see we found a user, otherwise return bad credentials
	let user = match user {
		Some(x) => x,
		None => return Err(APIError::BadCredentials),
	};

	// Check that our password matches the hash
	if !try500!(argon2::verify_encoded(&user.pass, query.pass.as_bytes())) {
		return Err(APIError::BadCredentials);
	}

	// Generate a token for the user
	let mut token = [0u8; 40];
	rand::thread_rng().fill(&mut token[..]);

	// Encode this into a key
	let mut key = String::with_capacity(64);
	key.push_str("user:");
	base64::encode_config_buf(token, base64::STANDARD, &mut key);

	// Don't bother checking if it's not taken, just error
	let user = user.into();
	auth_db.remember(&key, &user).await?;

	Ok(HttpResponse::Ok()
		.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
		.body(format!(
			r#"{{"success":"user logged in","token":"{}","data":{}}}"#,
			&key[5..key.len()],
			serde_json::to_string(&user).unwrap()
		)))
}

pub async fn delete_logout(
	req: HttpRequest,
	auth: Authenticated,
) -> Result<HttpResponse, APIError> {
	auth.forget(&req).await?;
	Ok(HttpResponse::Ok()
		.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
		.body(r#"{"success":"user logged out"}"#))
}

pub async fn get_self(
	pool: web::Data<DbPool>,
	auth: Authenticated,
) -> Result<HttpResponse, APIError> {
	let conn = try500!(pool.get().await, "post_login:db pool");
	let user = try500!(
		User::select_id::<pg::Client>(&conn, auth.uid).await,
		"post_login:select_name {:?}",
		auth.uid
	);
	let user = match user {
		Some(u) => UserAPI::from(u),
		None => return Err(APIError::BadRequestData),
	};

	Ok(HttpResponse::Ok()
		.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
		.body(serde_json::to_string(&user).unwrap()))
}

pub async fn get_logged_in(auth: MaybeAuthenticated) -> HttpResponse {
	if auth.is_authenticated() {
		HttpResponse::Ok()
			.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
			.body(r#"{"status": "logged in"}"#)
	} else {
		HttpResponse::Ok()
			.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
			.body(r#"{"status": "logged out"}"#)
	}
}
