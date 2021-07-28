pub mod post;
pub mod search;
pub mod tag;
// pub mod user;

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

pub async fn upload_post_html() -> Result<HttpResponse, crate::error::APIError> {
	Ok(HttpResponse::Ok()
		.append_header((header::CONTENT_TYPE, "text/html; charset=utf-8"))
		.body(UPLOAD_PAGE_HTML))
}
