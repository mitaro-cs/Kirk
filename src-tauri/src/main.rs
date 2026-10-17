// Без консольного окна на Windows в выпуске.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    kirk_lib::run()
}
