// Entry point for generating foreign-language bindings from the annotated core.
// Usage: cargo run --bin uniffi-bindgen -- generate --library <libevepass_core> --language swift ...
fn main() {
    uniffi::uniffi_bindgen_main()
}
