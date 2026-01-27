//! Route definitions.

/// Available API routes.
#[derive(Debug, Clone, Copy)]
pub enum Route {
    /// GET /users
    ListUsers,
    /// GET /users/:id
    GetUser,
    /// POST /users
    CreateUser,
    /// DELETE /users/:id
    DeleteUser,
    /// POST /login
    Login,
}

impl Route {
    /// Get the HTTP method for this route.
    pub fn method(&self) -> &'static str {
        match self {
            Route::ListUsers | Route::GetUser => "GET",
            Route::CreateUser | Route::Login => "POST",
            Route::DeleteUser => "DELETE",
        }
    }

    /// Get the path pattern for this route.
    pub fn path(&self) -> &'static str {
        match self {
            Route::ListUsers => "/users",
            Route::GetUser => "/users/:id",
            Route::CreateUser => "/users",
            Route::DeleteUser => "/users/:id",
            Route::Login => "/login",
        }
    }
}
