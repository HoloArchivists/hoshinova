fn main() {
    // Create the web/dist directory if it doesn't exist.
    let mut dist_dir = std::env::current_dir().unwrap();
    dist_dir.push("web");
    dist_dir.push("dist");
    if !dist_dir.exists() {
        std::fs::create_dir(&dist_dir).unwrap();
    }
}
