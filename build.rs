use std::process::Command;

fn main() {
    // Get current date/time at build time
    let output = Command::new("date")
        .arg("+%Y-%m-%d %H:%M:%S")
        .output()
        .expect("Failed to execute date command");
    
    let timestamp = String::from_utf8_lossy(&output.stdout).trim().to_string();
    
    println!("cargo:rustc-env=BUILD_TIMESTAMP={}", timestamp);
    println!("cargo:rerun-if-changed=build.rs");
}