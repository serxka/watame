use crate::database::{enums::Perms, user};
use crate::{error::APIError, try500};

use actix_web::{
	dev::{self, Service, ServiceRequest, ServiceResponse},
	http::header,
	web::Data,
	Error, HttpMessage, HttpRequest,
};
use futures::future::{ready, FutureExt, LocalBoxFuture, Ready};
use serde::{Deserialize, Serialize};

use std::rc::Rc;

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct AuthInfo {
	uid: i32,
	perms: Perms,
}

impl core::convert::From<user::User> for AuthInfo {
	fn from(user: user::User) -> Self {
		Self {
			uid: user.id,
			perms: user.perms,
		}
	}
}

// This isn't really a factory, but it's done this way so we don't have to use
// an Atomic reference counter
#[derive(Clone)]
pub struct AuthDbCreator {
	client: redis::Client,
	conn: redis::aio::MultiplexedConnection,
}

impl AuthDbCreator {
	pub async fn new(uri: &str) -> Self {
		let client = redis::Client::open(uri).expect("failed to create redis client");
		let conn = client
			.get_multiplexed_tokio_connection()
			.await
			.expect("failed to connect to redis");
		Self { client, conn }
	}

	pub async fn clear_sessions(uri: &str) {
		let mut auth_db = Self::new(uri).await;
		let _: () = redis::cmd("FLUSHALL")
			.query_async(&mut auth_db.conn)
			.await
			.expect("failed to flush redis keys");
	}
}

#[derive(Clone)]
pub struct AuthDb(Rc<AuthDbCreator>);

impl AuthDb {
	pub fn new(auth_db: AuthDbCreator) -> Self {
		Self(Rc::new(auth_db))
	}

	pub async fn remember(&self, key: &str, user: &AuthInfo) -> Result<bool, APIError> {
		let mut conn = self.0.conn.clone();
		let res: bool = try500!(
			redis::cmd("SETNX")
				.arg(key)
				.arg(serde_json::to_string(&user).unwrap())
				.query_async(&mut conn)
				.await,
			"authdb:remember SETNX {:?} {:?}",
			key,
			user
		);

		Ok(res)
	}

	pub async fn verify(
		&self,
		header: Option<&str>,
		_req: &ServiceRequest,
	) -> Result<Option<AuthInfo>, APIError> {
		if header.is_none() {
			return Ok(None);
		}
		// Check the token
		let token = header.unwrap();
		if token.len() == 0 || token.len() > 512 {
			return Err(APIError::BadRequestData);
		}
		let key = format!("user:{}", token);

		println!("{}", key);

		let mut conn = self.0.conn.clone();
		let exists: Option<String> = try500!(
			redis::cmd("GET").arg(&key).query_async(&mut conn).await,
			"authdb:verify GET {:?}",
			key
		);

		match exists {
			Some(v) => Ok(Some(serde_json::from_str(&v).unwrap())),
			None => Ok(None),
		}
	}
}

pub struct AuthMiddleware<S> {
	auth_db: AuthDb,
	service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for AuthMiddleware<S>
where
	S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
{
	type Response = ServiceResponse<B>;
	type Error = Error;
	type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

	actix_service::forward_ready!(service);

	fn call(&self, req: ServiceRequest) -> Self::Future {
		let srv = self.service.clone();
		let auth_db = self.auth_db.clone();

		async move {
			// Grab our authorization header, even if there isn't one
			let id = match req.headers().get(header::AUTHORIZATION) {
				Some(x) => Some(x.to_str().map_err(|_| APIError::BadRequestData)?),
				None => None,
			};
			// Get the database to check that it is valid
			let info = auth_db.verify(id, &req).await?;
			// If so insert an extension into the service request to get later
			if let Some(info) = info {
				req.extensions_mut().insert::<AuthInfo>(info);
			}

			let res = srv.call(req).await?;
			Ok(res)
		}
		.boxed_local()
	}
}

pub struct AuthMiddlewareFactory {
	auth_db: AuthDb,
}

impl AuthMiddlewareFactory {
	pub fn new(auth_db: AuthDb) -> Self {
		Self { auth_db }
	}
}

impl<S, B> dev::Transform<S, ServiceRequest> for AuthMiddlewareFactory
where
	S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
{
	type Response = ServiceResponse<B>;
	type Error = Error;
	type Transform = AuthMiddleware<S>;
	type InitError = ();
	type Future = Ready<Result<Self::Transform, Self::InitError>>;

	fn new_transform(&self, service: S) -> Self::Future {
		ready(Ok(AuthMiddleware {
			auth_db: self.auth_db.clone(),
			service: Rc::new(service),
		}))
	}
}

pub struct Authenticated(AuthInfo, AuthDb);

impl Authenticated {
	#[allow(dead_code)]
	pub fn get_db(&self) -> &AuthDb {
		&self.1
	}

	pub async fn forget(&self, req: &HttpRequest) -> Result<(), APIError> {
		// The idea is this is already checked, we are just getting it again
		let key = req
			.headers()
			.get(header::AUTHORIZATION)
			.unwrap()
			.to_str()
			.unwrap();

		let mut conn = self.1 .0.conn.clone();
		let _: () = try500!(
			redis::cmd("DEL").arg(&key).query_async(&mut conn).await,
			"auth:forget DEL {:?}",
			key
		);

		Ok(())
	}
}

impl actix_web::FromRequest for Authenticated {
	type Config = ();
	type Error = APIError;
	type Future = Ready<Result<Self, Self::Error>>;

	fn from_request(req: &HttpRequest, _payload: &mut dev::Payload) -> Self::Future {
		let val = req.extensions().get::<AuthInfo>().copied();
		let auth_db = req
			.app_data::<Data<AuthDb>>()
			.expect("AuthDb should be part of app_data")
			.get_ref()
			.clone();
		let res = match val {
			Some(v) => Ok(Authenticated(v, auth_db.clone())),
			None => Err(APIError::BadCredentials),
		};
		ready(res)
	}
}

impl core::ops::Deref for Authenticated {
	type Target = AuthInfo;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

pub struct MaybeAuthenticated(Option<AuthInfo>, AuthDb);

impl MaybeAuthenticated {
	#[allow(dead_code)]
	pub fn get_db(&self) -> &AuthDb {
		&self.1
	}
}

impl actix_web::FromRequest for MaybeAuthenticated {
	type Config = ();
	type Error = APIError;
	type Future = Ready<Result<Self, Self::Error>>;

	fn from_request(req: &HttpRequest, _payload: &mut dev::Payload) -> Self::Future {
		let val = req.extensions().get::<AuthInfo>().copied();
		let auth_db = req
			.app_data::<Data<AuthDb>>()
			.expect("AuthDb should be part of app_data")
			.get_ref()
			.clone();
		ready(Ok(MaybeAuthenticated(val, auth_db.clone())))
	}
}

impl core::ops::Deref for MaybeAuthenticated {
	type Target = Option<AuthInfo>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl MaybeAuthenticated {
	pub fn is_authenticated(&self) -> bool {
		self.0.is_some()
	}

	#[allow(dead_code)]
	pub fn into_authinfo(self) -> AuthInfo {
		match self.0 {
			Some(x) => x,
			None => panic!(),
		}
	}
}
