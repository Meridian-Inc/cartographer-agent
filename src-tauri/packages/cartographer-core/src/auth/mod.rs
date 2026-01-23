//! Authentication module for Cartographer agents.
//!
//! Provides OAuth 2.0 device flow authentication and secure credential storage.

mod credentials;
mod device_flow;

pub use credentials::{
    check_auth, delete_credentials, get_credential_storage_info, load_credentials,
    save_credentials, AuthStatus, Credentials,
};
pub use device_flow::{
    poll_for_login, request_login_url, start_login, LoginFlowStarted, LoginUrlEvent,
};
