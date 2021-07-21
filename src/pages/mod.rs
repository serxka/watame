pub mod post;
pub mod search;
// pub mod user;

use crate::error::APIError;

pub fn error500(msg: &str, err: Box<(dyn std::error::Error + Sync + Send + 'static)>) -> APIError {
	log::error!(
		"internal error has occurred!\n[MESSAGE]: {}\n[ERROR]: {:?}",
		msg,
		err
	);
	APIError::InternalError
}

use actix_web::{http::header, HttpResponse};

const UPLOAD_PAGE_HTML: &'static str = r#"
<!DOCTYPE html>
<html>
<head>
	<title>Upload</title>
</head>
<body>
<form method="POST" action="/post" enctype="multipart/form-data">
	<input type="file" name="image">
	<input type="textarea" name="data">
	<button type="submit">Submit</button>
</form>
</body>
</html>
"#;

pub async fn upload_post_html() -> Result<HttpResponse, APIError> {
	Ok(HttpResponse::Ok()
		.append_header((header::CONTENT_TYPE, "text/html; charset=utf-8"))
		.body(UPLOAD_PAGE_HTML))
}
