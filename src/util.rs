#[macro_export]
macro_rules! log {
    ($($arg:expr),+) => {
        #[cfg(target_arch = "wasm32")]
        gloo_console::log!([$($arg,)+]);
        #[cfg(not(target_arch = "wasm32"))]
        log::info!("{}", vec![$(String::from($arg),)+].join(" "));
    };
}
