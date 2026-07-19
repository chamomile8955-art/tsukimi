#![cfg_attr(
    all(target_os = "windows", not(debug_assertions), not(feature = "console")),
    windows_subsystem = "windows"
)]

fn main() -> gtk::glib::ExitCode {
    tsukimi::run()
}
