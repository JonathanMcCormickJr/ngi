use axum::{
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub role: String,
}

#[derive(Clone)]
pub struct AuthState {
    pub jwt_secret: Vec<u8>,
}

pub async fn auth_middleware(
    State(state): State<Arc<AuthState>>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok());

    let Some(auth_header) = auth_header else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    if !auth_header.starts_with("Bearer ") {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token = &auth_header[7..];

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(&state.jwt_secret),
        &Validation::default(),
    )
    .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Insert claims into request extensions so handlers can access them
    req.extensions_mut().insert(token_data.claims);

    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::Body;
    use axum::http::{Request as HttpRequest, StatusCode as HttpStatusCode};
    use axum::middleware;
    use axum::response::IntoResponse;
    use axum::routing::get;
    use jsonwebtoken::{EncodingKey, Header, encode};
    use tower::ServiceExt;

    async fn claims_handler(req: HttpRequest<Body>) -> impl IntoResponse {
        if req.extensions().get::<Claims>().is_some() {
            HttpStatusCode::OK
        } else {
            HttpStatusCode::INTERNAL_SERVER_ERROR
        }
    }

    fn test_app(secret: &[u8]) -> Router {
        let auth_state = Arc::new(AuthState {
            jwt_secret: secret.to_vec(),
        });

        Router::new()
            .route("/protected", get(claims_handler))
            .layer(middleware::from_fn_with_state(auth_state, auth_middleware))
    }

    fn test_token(secret: &[u8]) -> String {
        let claims = Claims {
            sub: "user-123".to_string(),
            exp: 4_102_444_800,
            role: "Admin".to_string(),
        };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret),
        )
        .expect("token generation")
    }

    #[tokio::test]
    async fn missing_authorization_header_returns_unauthorized() {
        let app = test_app(b"secret");
        let response = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/protected")
                    .body(Body::empty())
                    .expect("request build"),
            )
            .await
            .expect("router call");

        assert_eq!(response.status(), HttpStatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn non_bearer_authorization_header_returns_unauthorized() {
        let app = test_app(b"secret");
        let response = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/protected")
                    .header(header::AUTHORIZATION, "Token abc")
                    .body(Body::empty())
                    .expect("request build"),
            )
            .await
            .expect("router call");

        assert_eq!(response.status(), HttpStatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn valid_bearer_token_allows_request_and_injects_claims() {
        let secret = b"secret";
        let token = test_token(secret);
        let app = test_app(secret);

        let response = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/protected")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .expect("request build"),
            )
            .await
            .expect("router call");

        assert_eq!(response.status(), HttpStatusCode::OK);
    }
}
