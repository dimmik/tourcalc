//! Issues or checks a token with the Rust implementation, for comparing against C#.
//!
//! usage: cargo run -p tc-server --example token -- issue
//!        cargo run -p tc-server --example token -- check <token>

use tc_server::auth;

fn main() {
    const DEV_KEY: &str = "aSXx0m1XH4K1GfIYR8mi7/XrSWGCH30Eqn074DhewZo=";
    let signer = auth::Signer_::from_base64(DEV_KEY).unwrap();
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("issue") => {
            let a = auth::AuthData::for_code_md5("ABC");
            println!("{}", signer.issue("code", &a, 60));
        }
        Some("check") => {
            let token = args.next().expect("token");
            match signer.verify(&token) {
                Ok(a) => println!(
                    "valid. AuthDataJson = {}",
                    serde_json::to_string(&a).unwrap()
                ),
                Err(e) => {
                    println!("REJECTED: {e}");
                    std::process::exit(1);
                }
            }
        }
        _ => eprintln!("usage: token issue | token check <jwt>"),
    }
}
