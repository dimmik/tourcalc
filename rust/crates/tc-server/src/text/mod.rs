//! The text interface: plain HTML for browsers that run no JavaScript.
//!
//! The app is a WebAssembly client, which lynx, w3m and links cannot run at all - and a
//! phone on a mountain pass with one bar is not much better off. These pages are the same
//! tours rendered as tables and forms on the server, with the arithmetic coming from the
//! same `tc-core`, so the two interfaces cannot quote different numbers.
//!
//! Everything here has to mean something without CSS: headings are headings, tables are
//! tables, and every action is a link or a form button. The small stylesheet is only so the
//! pages are bearable in a graphical browser; nothing about them depends on it.
//!
//! **Signing in is a cookie**, because a text browser has no way to put a token in an
//! Authorization header. It holds the ordinary Tourcalc token, it is `HttpOnly`, and it is
//! `SameSite=Strict` - these pages change data through plain form posts, so strict costs
//! nothing here and closes the cross-site request the cookie would otherwise enable. The
//! JSON API never reads it, so a cookie still cannot speak for anybody there.

mod forms;
mod pages;
pub mod redirect;
mod render;

use crate::auth::AuthData;
use crate::state::Shared;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::routing::{get, post};
use axum::Router;

pub use render::{esc, money, page};

/// The name of the cookie the text pages are signed in with.
pub const COOKIE: &str = "tc_text";

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/t", get(pages::index))
        .route("/t/login", get(pages::login_form).post(pages::login))
        .route("/t/logout", get(pages::logout))
        // The same shape as the app's share link, so a link already in somebody's hands
        // works by adding the /t.
        .route("/t/goto/{md5}", get(pages::goto))
        .route("/t/goto/{md5}/{tour}", get(pages::goto))
        .route("/t/{id}", get(pages::tour))
        .route("/t/{id}/markpaid", post(pages::mark_paid))
        .route("/t/{id}/people", get(pages::people))
        .route(
            "/t/{id}/people/edit",
            get(pages::person_form).post(pages::person_save),
        )
        .route(
            "/t/{id}/people/edit/{person}",
            get(pages::person_form).post(pages::person_save),
        )
        .route("/t/{id}/people/delete/{person}", post(pages::person_delete))
        .route("/t/{id}/spend", get(pages::spend))
        .route(
            "/t/{id}/spend/edit",
            get(pages::spending_form).post(pages::spending_save),
        )
        .route(
            "/t/{id}/spend/edit/{spending}",
            get(pages::spending_form).post(pages::spending_save),
        )
        .route(
            "/t/{id}/spend/delete/{spending}",
            post(pages::spending_delete),
        )
        .route("/t/{id}/stats", get(pages::stats))
}

/// Who is reading, according to the cookie.
///
/// Never a rejection: somebody with no cookie is nobody, and a page sends them to the login
/// form rather than answering 401 - which a text browser shows as an error page with no way
/// out of it.
pub struct Reader(pub AuthData);

impl Reader {
    pub fn signed_in(&self) -> bool {
        self.0.kind != "None"
    }
}

impl FromRequestParts<Shared> for Reader {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Shared,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(axum::http::header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .and_then(|header| {
                header.split(';').find_map(|pair| {
                    let (name, value) = pair.split_once('=')?;
                    (name.trim() == COOKIE).then(|| value.trim().to_owned())
                })
            });

        let auth = match token {
            Some(t) => state.signer.verify(&t).unwrap_or_default(),
            None => AuthData::default(),
        };
        Ok(Reader(auth))
    }
}
