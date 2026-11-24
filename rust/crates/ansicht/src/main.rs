//! Entry point of the browser view. wasm-bindgen calls `main` when the module starts; on the host it does nothing.

fn main() {
    #[cfg(target_arch = "wasm32")]
    dioxus_web::launch::launch_cfg(
        ansicht::oberflaeche::app,
        dioxus_web::Config::new().rootname("wurzel"),
    );
}
