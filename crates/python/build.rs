//! Build-time linker configuration for PyO3 extension module.

fn main() {
    // Only apply extension-module linker args when that feature is enabled.
    // This keeps normal Rust test binaries linkable (they need libpython)
    // while wheel builds still get the correct extension-module behavior.
    if std::env::var_os("CARGO_FEATURE_EXTENSION_MODULE").is_some() {
        pyo3_build_config::add_extension_module_link_args();
    }
}
