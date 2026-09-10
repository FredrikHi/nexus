use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use utoipa::ToSchema;

/// Application-wide error type. Every layer returns `Result<_, ApiError>`, and
/// the `?` operator converts lower-level errors into this via the `From` impls
/// below. This is the Rust replacement for throwing exceptions across layers.
#[derive(Debug)]
pub enum ApiError {
    NotFound(String),
    /// No usable credential was presented, or it is revoked or expired.
    Unauthorized(String),
    /// The caller is known, but not allowed to do this. Distinct from
    /// Unauthorized: signing in again would not help.
    Forbidden(String),
    /// The request was malformed: unparseable JSON, an unusable path or query
    /// parameter. Distinct from `Validation`, which means "well-formed, but the
    /// values are wrong".
    BadRequest(String),
    UnsupportedMediaType(String),
    Validation(String),
    Conflict(String),
    Internal(String),
}

/// The response body every error returns, whatever layer produced it.
#[derive(Serialize, ToSchema)]
pub struct ErrorBody {
    pub error: ErrorDetail,
}

#[derive(Serialize, ToSchema)]
pub struct ErrorDetail {
    /// Stable machine-readable code, e.g. `NOT_FOUND`.
    #[schema(example = "VALIDATION_FAILED")]
    pub code: &'static str,
    /// Human-readable detail. Redacted for 5xx.
    pub message: String,
}

impl ApiError {
    fn status_and_code(&self) -> (StatusCode, &'static str) {
        match self {
            ApiError::NotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            ApiError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED"),
            ApiError::Forbidden(_) => (StatusCode::FORBIDDEN, "FORBIDDEN"),
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, "BAD_REQUEST"),
            ApiError::UnsupportedMediaType(_) => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "UNSUPPORTED_MEDIA_TYPE",
            ),
            ApiError::Validation(_) => (StatusCode::UNPROCESSABLE_ENTITY, "VALIDATION_FAILED"),
            ApiError::Conflict(_) => (StatusCode::CONFLICT, "CONFLICT"),
            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::NotFound(m)
            | ApiError::Unauthorized(m)
            | ApiError::Forbidden(m)
            | ApiError::BadRequest(m)
            | ApiError::UnsupportedMediaType(m)
            | ApiError::Validation(m)
            | ApiError::Conflict(m)
            | ApiError::Internal(m) => write!(f, "{m}"),
        }
    }
}

/// Marks `ApiError` as a real error type. Display and Debug are already here,
/// which is all this trait needs, and it is what lets `?` convert an `ApiError`
/// into an `anyhow::Error` in `main`. Roughly: deriving from Exception in C#.
impl std::error::Error for ApiError {}

/// The trait that lets Axum turn an `ApiError` returned from a handler straight
/// into an HTTP response. Because of this, handlers can just `return Err(...)`.
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = self.status_and_code();

        // 5xx: log the real detail server-side, never leak it to the client.
        if status.is_server_error() {
            tracing::error!(error = %self, "internal error");
        }
        let message = if status.is_server_error() {
            "An internal error occurred.".to_string()
        } else {
            self.to_string()
        };

        (status, Json(ErrorBody { error: ErrorDetail { code, message } })).into_response()
    }
}

/// This is what makes `?` work on repository calls: when a query returns
/// `Err(sqlx::Error)`, `?` invokes this conversion. We turn known constraint
/// violations into meaningful HTTP codes and everything else into a 500.
impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        if let sqlx::Error::Database(db) = &err {
            if db.is_unique_violation() {
                return ApiError::Conflict("A resource with the same unique value already exists.".to_string());
            }
            if db.is_foreign_key_violation() {
                return ApiError::Validation("A referenced resource does not exist.".to_string());
            }
        }
        ApiError::Internal(format!("database error: {err}"))
    }
}

// ---------------------------------------------------------------------------
// Extractor rejections
//
// Axum's own extractors run BEFORE a handler body and answer with their own
// plain-text responses, so nothing our handlers return can influence them.
// The wrappers in `crate::extract` re-declare their `Rejection` type as
// `ApiError`, and these conversions are what that re-declaration relies on.
//
// The rejection text is passed through rather than replaced: these are all 4xx,
// where naming the offending field is exactly what a client needs. Only 5xx
// detail is redacted, in `IntoResponse` above.
// ---------------------------------------------------------------------------

impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        match rejection {
            // Valid JSON whose shape or values do not fit the target type,
            // e.g. an unknown enum token. Same 422 as our own validation.
            JsonRejection::JsonDataError(e) => ApiError::Validation(e.body_text()),
            // Not valid JSON at all.
            JsonRejection::JsonSyntaxError(e) => ApiError::BadRequest(e.body_text()),
            JsonRejection::MissingJsonContentType(e) => {
                ApiError::UnsupportedMediaType(e.body_text())
            }
            // `JsonRejection` is #[non_exhaustive], so a wildcard is required:
            // a future axum release may add variants without a major version.
            other => ApiError::BadRequest(other.body_text()),
        }
    }
}

impl From<PathRejection> for ApiError {
    fn from(rejection: PathRejection) -> Self {
        match rejection {
            // The URL carried something we could not turn into the target type,
            // e.g. `/systems/not-a-uuid`.
            PathRejection::FailedToDeserializePathParams(e) => ApiError::BadRequest(e.body_text()),
            // The route pattern and the handler signature disagree. That is our
            // bug, not the caller's, so it is a 500 and gets logged.
            other => ApiError::Internal(format!("path extraction failed: {}", other.body_text())),
        }
    }
}

impl From<QueryRejection> for ApiError {
    fn from(rejection: QueryRejection) -> Self {
        ApiError::BadRequest(rejection.body_text())
    }
}
