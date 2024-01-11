use actix_web::{http::header::ContentType, HttpResponse};
use actix_web_flash_messages::IncomingFlashMessages;
use std::fmt::Write;

pub async fn login_form(flash_messages: IncomingFlashMessages) -> HttpResponse {
    let mut error_html = String::new();
    for m in flash_messages.iter() {
        writeln!(error_html, "<p><i>{}</i></p>", m.content()).unwrap();
    }
    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta http-equiv="content-type" content="text/html; charset=utf-8">
    <title>Login</title>
    <link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/materialize/1.0.0/css/materialize.min.css">
</head>
<body>
<div class="container">
    {error_html}
    <div class="row">
    <form action="/login" method="post" class="col s12">
    <div class="row">
    <div class="input-field col s12">
          <input placeholder="Username" id="username" name="username" type="text" class="validate">
          <label for="username">Username</label>
        </div>
        </div>
        <div class="row">
    <div class="input-field col s12">
            <input placeholder="Password" id="password" name="password" type="password" class="validate">
            <label for="username">Password</label>
        </div>
        </div>
        <button class="waves-effect waves-light btn" type="submit">Login</button>
    </form>
    </div>
    </div>
    <!-- Compiled and minified JavaScript -->
    <script src="https://cdnjs.cloudflare.com/ajax/libs/materialize/1.0.0/js/materialize.min.js"></script>
</body>
</html>"#,
        ))
}
