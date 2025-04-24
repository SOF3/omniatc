mod derive_config;

#[proc_macro_derive(Config, attributes(config))]
pub fn derive_config(ts: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive_config::impl_(ts.into()).unwrap_or_else(syn::Error::into_compile_error).into()
}
