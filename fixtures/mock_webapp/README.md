# Mock Web Application

A simple web API for demonstration purposes.

## Features

- User management
- Session handling
- Authentication

## Getting Started

```bash
cargo build
cargo test
```

## Configuration

Set environment variables or use config file.

## API Endpoints

- `GET /users/{id}`: Retrieves a user by their unique ID. Returns user object if found.
- `POST /users`: Creates a new user with name and email. Returns created user on success.
- `POST /login`: Authenticates a user with email and password. Returns authentication token on success.
- `GET /users`: Returns a list of all users in the system.
- `DELETE /users/{id}`: Deletes a user by their ID. Returns success status.

