fn main() {
    // Create the web/dist directory if it doesn't exist.
    let mut dist_dir = std::env::current_dir().unwrap();
    dist_dir.push("web");
    dist_dir.push("dist");
    if !dist_dir.exists() {
        std::fs::create_dir(&dist_dir).unwrap();
    }

    // Get the current git commit hash
    let output = std::process::Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .output()
        .expect("Failed to execute git");
    let git_hash = String::from_utf8(output.stdout).unwrap();
    println!("cargo:rustc-env=GIT_HASH={}", git_hash);
    println!(
        "cargo:rustc-env=GIT_HASH_SHORT={}",
        git_hash[..7].to_string()
    );

    // Build time
    let now = chrono::Utc::now();
    println!("cargo:rustc-env=BUILD_TIME={}", now.to_rfc3339());
}
