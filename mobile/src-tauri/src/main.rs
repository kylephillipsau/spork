// The desktop entry point. On iOS and Android the shell calls `run()` through
// `mobile_entry_point` instead and this file is not compiled in.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    nylonite_mobile_lib::run()
}
