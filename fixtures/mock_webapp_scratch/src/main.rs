//! Mock Web Application entry point.

mod api;
mod config;
mod models;
mod services;
mod utils;

fn main() {
    let config = config::Config::load();
    println!("Starting server with config: {:?}", config);

    // Initialize services
    let _auth = services::auth::AuthService::new(&config);
    let _db = services::database::Database::new(&config);

    println!("Server ready!");
}
