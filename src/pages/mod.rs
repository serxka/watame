pub mod post;
pub mod search;

use crate::error::APIError;

pub fn error500(msg: &str, err: Box<(dyn std::error::Error + Sync + Send + 'static)>) -> APIError {
	log::error!(
		"internal error has occurred!\n[MESSAGE]: {}\n[ERROR]: {:?}",
		msg,
		err
	);
	APIError::InternalError
}
