pub fn initialize_logger() {
    {
        // Initialize the logger only once (useful when running the driver in tests).
        use std::sync::atomic::{AtomicBool, Ordering};
        static LOGGER_INITIALIZED: AtomicBool = AtomicBool::new(false);
        if LOGGER_INITIALIZED.swap(true, Ordering::SeqCst) {
            return;
        }
    }

    use std::io::IsTerminal;
    use tracing_subscriber::prelude::*;
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(
            tracing_tree::HierarchicalLayer::new(1)
                .with_ansi(std::io::stderr().is_terminal())
                .with_indent_lines(true)
                .with_bracketed_fields(true)
                .with_timer(tracing_tree::time::Uptime::default()),
        )
        .init();
}
