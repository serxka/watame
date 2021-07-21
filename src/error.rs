use actix_web::{
	dev::BaseHttpResponseBuilder, error, http::header, http::StatusCode, HttpResponse,
};
use derive_more::{Display, Error};

#[allow(unused)]
#[derive(Debug, Display, Error)]
pub enum APIError {
	#[display(fmt = r#"{{"error":"internal server error"}}"#)]
	InternalError,
	#[display(fmt = r#"{{"error":"bad request"}}"#)]
	BadRequestData,
	#[display(fmt = r#"{{"error":"timeout"}}"#)]
	Timeout,
	#[display(fmt = r#"{{"error":"unauthorised"}}"#)]
	Auth,
	#[display(fmt = r#"{{"error":"payload to large"}}"#)]
	PayloadSize,
	#[display(fmt = r#"{{"error":"unsupported mime type"}}"#)]
	MimeType,
	#[display(fmt = r#"{{"error":"too many tags, please reduce amount and try again"}}"#)]
	TagLimit,
	#[display(fmt = r#"{{"error":"one or more tags contained invalid characters"}}"#)]
	BadTags,
	#[display(
		fmt = r#"{{"error":"too many items per page request, please reduce amount and try again"}}"#
	)]
	PageSize,
}

impl error::ResponseError for APIError {
	fn error_response(&self) -> HttpResponse {
		BaseHttpResponseBuilder::new(self.status_code())
			.insert_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
			.body(self.to_string())
			.into()
	}

	fn status_code(&self) -> StatusCode {
		match *self {
			Self::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
			Self::BadRequestData => StatusCode::BAD_REQUEST,
			Self::Timeout => StatusCode::GATEWAY_TIMEOUT,
			Self::Auth => StatusCode::UNAUTHORIZED,
			Self::PayloadSize => StatusCode::PAYLOAD_TOO_LARGE,
			Self::MimeType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
			Self::TagLimit => StatusCode::BAD_REQUEST,
			Self::BadTags => StatusCode::BAD_REQUEST,
			Self::PageSize => StatusCode::BAD_REQUEST,
		}
	}
}
