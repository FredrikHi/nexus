//! Extractors that fail into `ApiError` instead of Axum's plain-text defaults.
//!
//! Axum runs extractors before the handler body, so an unparseable body or a
//! malformed UUID never reaches our code and never touches
//! `impl IntoResponse for ApiError`. The result was a client that could not
//! call `res.json()` on exactly the errors it hits most often.
//!
//! Every extractor declares an associated `Rejection` type, which is the hook
//! for overriding that. These wrappers delegate the real work to Axum's own
//! extractor and then convert its rejection via `From` (see `crate::error`).
//! In ASP.NET this is the job of `InvalidModelStateResponseFactory`; Rust has
//! no central hook, so it is done per extractor.
//!
//! The wrappers deliberately reuse Axum's names. Swap the import at the top of
//! a handlers file and every signature in it stays byte for byte identical.

use axum::extract::{FromRequest, FromRequestParts, Request};
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::error::ApiError;

/// JSON body extractor, and the JSON response wrapper.
///
/// It is both because handlers use `Json` in both positions: as an argument to
/// parse the request, and in the return type to render the response. Only the
/// request half needed fixing, so the response half just delegates.
pub struct Json<T>(pub T);

impl<S, T> FromRequest<S> for Json<T>
where
    axum::Json<T>: FromRequest<S, Rejection = axum::extract::rejection::JsonRejection>,
    S: Send + Sync,
{
    // This one line is the whole point of the module.
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        // `?` converts JsonRejection into ApiError through the From impl.
        let axum::Json(value) = axum::Json::<T>::from_request(req, state).await?;
        Ok(Json(value))
    }
}

impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

/// Path parameter extractor, e.g. the `{id}` in `/systems/{id}`.
///
/// `FromRequestParts` rather than `FromRequest`: it reads only the URL and
/// headers, never the body. That is what lets it sit before a `Json` argument
/// in a handler signature, since only one extractor may consume the body.
pub struct Path<T>(pub T);

impl<S, T> FromRequestParts<S> for Path<T>
where
    axum::extract::Path<T>:
        FromRequestParts<S, Rejection = axum::extract::rejection::PathRejection>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let axum::extract::Path(value) =
            axum::extract::Path::<T>::from_request_parts(parts, state).await?;
        Ok(Path(value))
    }
}

/// Query string extractor, e.g. `?system_id=...`.
pub struct Query<T>(pub T);

impl<S, T> FromRequestParts<S> for Query<T>
where
    axum::extract::Query<T>:
        FromRequestParts<S, Rejection = axum::extract::rejection::QueryRejection>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let axum::extract::Query(value) =
            axum::extract::Query::<T>::from_request_parts(parts, state).await?;
        Ok(Query(value))
    }
}
