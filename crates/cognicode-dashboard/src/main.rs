//! Main entry point for the CogniCode Dashboard
//!
//! This is a Leptos CSR (Client-Side Rendered) application.

use leptos::prelude::*;
use cognicode_dashboard::app::App;

fn main() {
    // Mount the app to the body
    mount_to_body(App);
}
