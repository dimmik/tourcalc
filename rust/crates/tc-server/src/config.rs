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

    /// How many tours one access code may hold. `-1` is no limit, which is the default the
    /// C# ships with.
    pub max_tours_per_code: i64,
    /// Whether saving a tour keeps the state it replaced.
    pub versioning: bool,
    /// Whether a stored version may itself be written to. Off, as in the C#: a version is a
    /// record of what was, and editing one would make it a record of nothing.
    pub version_editable: bool,
    /// The word this build calls itself, echoed in `X-Tourcalc-Version`.
    pub build_type: String,
    /// The secret in the wake-up URL. Not authentication - it is one string in a path, and
    /// it only guards an endpoint that does nothing but wait.
    pub wakeup_code: String,
    pub wakeup_pre_delay_min: u64,
    pub wakeup_post_delay_min: u64,
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
            max_tours_per_code: number("MaxCountOfToursPerCode", -1),
            versioning: flag("TourVersioning", true),
            version_editable: flag("TourVersionEditable", false),
            build_type: var("BUILD_TYPE").unwrap_or_else(|| "na".to_owned()),
            wakeup_code: var("WakeupCode").unwrap_or_else(|| "secCode".to_owned()),
            wakeup_pre_delay_min: number("WaketimePreDelayInMin", 1) as u64,
            wakeup_post_delay_min: number("WaketimePostDelayInMin", 1) as u64,
        }
    }
}

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

fn number(name: &str, default: i64) -> i64 {
    var(name)
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

/// A boolean the way .NET configuration reads one: "true"/"false", any case.
fn flag(name: &str, default: bool) -> bool {
    var(name)
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}
