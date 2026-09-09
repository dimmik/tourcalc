//! Settings, read from the environment under the names the C# app already uses.
//!
//! Deliberately no config file: everything the deployment sets, it sets as an environment
//! variable today, and inventing a second way to say the same thing is how the two get out
//! of step.

pub struct Config {
    pub master_key: String,
    pub private_key_b64: String,
    pub token_valid_minutes: i64,
    pub seed_file: String,
    /// Where the built Blazor client lives, if it is to be served from here as well.
    pub static_dir: Option<String>,
    pub listen: String,
}

impl Config {
    pub fn from_env() -> Config {
        Config {
            master_key: var("MasterKey").unwrap_or_default(),
            // The development key from appsettings.json, so `cargo run` works with the
            // tokens the development server already handed out. A deployment sets its own,
            // and the app refuses to start without one if this default is removed.
            private_key_b64: var("AuthPrivateECDSAKey")
                .unwrap_or_else(|| "aSXx0m1XH4K1GfIYR8mi7/XrSWGCH30Eqn074DhewZo=".to_owned()),
            token_valid_minutes: var("TokenValidTimeInMinutes")
                .and_then(|v| v.parse().ok())
                .unwrap_or(180 * 60 * 24),
            seed_file: var("InMemoryFileName")
                .unwrap_or_else(|| "TCBlazor/Server/inmemory-tours.json".to_owned()),
            static_dir: var("StaticFilesDir"),
            listen: var("Listen").unwrap_or_else(|| "127.0.0.1:5400".to_owned()),
        }
    }
}

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}
