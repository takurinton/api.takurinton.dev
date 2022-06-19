mod resolvers;

use axum::{
    extract::Extension,
    http::{HeaderValue, Method},
    response::{Html, IntoResponse},
    routing::get,
    Json, 
    Router,
};
use async_graphql::{
    http::{playground_source, GraphQLPlaygroundConfig},
    EmptyMutation,
    EmptySubscription,
    Request,
    Response,
    Schema,
};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use resolvers::QueryRoot;

pub type BlogSchema = Schema<QueryRoot, EmptyMutation, EmptySubscription>;

async fn graphql_handler(schema: Extension<BlogSchema>, req: Json<Request>) -> Json<Response> {
    schema.execute(req.0).await.into()
}

async fn graphql_playground() -> impl IntoResponse {
    Html(playground_source(GraphQLPlaygroundConfig::new("/")))
}

#[tokio::main]
async fn main() {
    let server = async {
        let schema = Schema::build(QueryRoot, EmptyMutation, EmptySubscription)
        .finish();

        let app = Router::new().route("/", get(graphql_playground).post(graphql_handler))
        .layer(
            CorsLayer::new()
                // 一旦現段階で想定してるのはブログだけ       
                .allow_origin("*".parse::<HeaderValue>().unwrap())
                .allow_methods([Method::GET, Method::POST, Method::OPTIONS]),
        )
        .layer(Extension(schema));
    
        let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
        axum::Server::bind(&addr)
            .serve(app.into_make_service())
            .await
            .unwrap();
    };

    tokio::join!(server);
}
