use crate::session_state::TypedSession;
use crate::utils::e500;
use actix_web::http::header::LOCATION;
use actix_web::{http::header::ContentType, web, HttpResponse};

pub async fn admin_dashboard(session: TypedSession) -> Result<HttpResponse, actix_web::Error> {
    let username = if let Some(user_id) = session.get_user_id().map_err(e500)? {
        user_id
    } else {
        return Ok(HttpResponse::SeeOther()
            .insert_header((LOCATION, "/login"))
            .finish());
    };
    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta http-equiv="content-type" content="text/html; charset=utf-8">
    <title>Admin dashboard</title>
</head>
<body>
    <p>Welcome {username}!</p>
    <p>Available actions:</p>
    <ol>
        <li><a href="/admin/password">Change password</a></li>
        <li>
          <form name="logoutForm" action="/admin/logout" method="post">
            <input type="submit" value="Logout">
          </form>
        </li>
    </ol>
</body>
</html>"#,
        )))
}
