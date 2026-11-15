// sqlx::migrate! embeds ./migrations at compile time: a new file must rebuild.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
