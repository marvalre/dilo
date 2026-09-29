#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some(dilo_lib::engine::WORKER_FLAG) {
        let dir = std::path::PathBuf::from(args.get(2).expect("model dir"));
        dilo_lib::engine::worker_main(&dir);
    }
    dilo_lib::run()
}
