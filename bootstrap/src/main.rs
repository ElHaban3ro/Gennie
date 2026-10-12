mod registry;
mod startup;
fn main() {
    if let Err(error) = startup::run() {
        eprintln!("event=startup_failed error={error}");
        std::process::exit(1);
    }
}
