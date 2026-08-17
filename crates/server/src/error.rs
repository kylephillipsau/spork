use actix_web::{http::StatusCode, HttpResponse, ResponseError};

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("database error: {0}")]
    Database(#[from] tokio_postgres::Error),

    #[error("connection pool error: {0}")]
    Pool(#[from] deadpool_postgres::PoolError),

    /// Raised at startup, not per request. A misconfigured server should refuse
    /// to start rather than serve wrong answers quietly.
    #[error("configuration error: {0}")]
    Configuration(String),

    #[error("no tenant on the request")]
    NoTenant,

    /// No session, or one that has expired, been revoked, or whose membership
    /// has lapsed. **One class for all of them**: telling a caller which would
    /// say whether an account exists, which OWASP asks us not to.
    #[error("not signed in")]
    Unauthenticated,

    #[error("not found")]
    NotFound,

    /// A write the client asked for is structurally invalid. The body carries
    /// the problem list; this is the class only.
    #[error("rejected: {0}")]
    Rejected(String),
}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::NoTenant | ApiError::Rejected(_) => StatusCode::BAD_REQUEST,
            ApiError::Unauthenticated => StatusCode::UNAUTHORIZED,
            ApiError::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        // The message goes to the log; the client gets the class. A database
        // error text can carry column names, constraint names and sometimes row
        // values, and none of that belongs in a response to a browser.
        if self.status_code().is_server_error() {
            tracing::error!(error = %self, "request failed");
        }
        let (error, detail) = match self {
            ApiError::NoTenant => ("no tenant on the request", None),
            ApiError::Unauthenticated => ("not signed in", None),
            ApiError::NotFound => ("not found", None),
            ApiError::Rejected(d) => ("rejected", Some(d.as_str())),
            _ => ("internal error", None),
        };
        let mut body = serde_json::json!({ "error": error });
        if let Some(d) = detail {
            body["detail"] = serde_json::json!(d);
        }
        HttpResponse::build(self.status_code()).json(body)
    }
}
