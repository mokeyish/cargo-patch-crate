pub fn main() {
    let _ = paris::Logger::new();
    if let Err(err) = patch_crate::run() {
        eprintln!("ERROR: {}", err);
        err.chain()
            .skip(1)
            .for_each(|cause| eprintln!("because: {}", cause));
        std::process::exit(1);
    }
}
