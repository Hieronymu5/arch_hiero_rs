//! Records when the app was built, as `ARCH_HIERO_BUILT` (an RFC 3339 timestamp), for the home
//! page. `SOURCE_DATE_EPOCH` sets it instead, for reproducible builds.

fn main() {
    let built = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|s| s.trim().parse::<i64>().ok())
        .and_then(|seconds| jiff::Timestamp::from_second(seconds).ok())
        .unwrap_or_else(jiff::Timestamp::now);
    println!("cargo:rustc-env=ARCH_HIERO_BUILT={built}");
    // Built again whenever any crate's code or the dependencies change, so the date is the
    // last time the app itself changed.
    for path in [
        "build.rs",
        "../../crates",
        "../../Cargo.toml",
        "../../Cargo.lock",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
}
