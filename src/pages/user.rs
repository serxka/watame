use crate::auth::{AuthDb, Authenticated, MaybeAuthenticated};
use crate::database::{
	user::{NewUser, User},
	Pool as DbPool,
};
use crate::{error::APIError, try500};

use actix_web::{http::header, web, HttpRequest, HttpResponse};
use argon2::{self, Config};

use rand::Rng;

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
	let conn = try500!(pool.get().await, "post_register:db pool");
	if try500!(
		User::check_existence(&conn, &query.user, Some(&query.email)).await,
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
		new_user.insert_into(&conn).await,
		"post_register:insert_into {:?}",
		new_user
	);

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
		User::select_name(&conn, &query.user).await,
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
	auth_db.remember(&key, &user.into()).await?;

	// Re-encode without 'user:' to send to client as a token
	key.clear();
	base64::encode_config_buf(token, base64::STANDARD, &mut key);

	Ok(HttpResponse::Ok()
		.append_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
		.body(format!(
			r#"{{"success":"user logged in","token":"{}"}}"#,
			key
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
