#[macro_export]
macro_rules! console_log {
    ($($t:tt)*) => (console::log_1(&format_args!($($t)*).to_string().as_str().into()))
}