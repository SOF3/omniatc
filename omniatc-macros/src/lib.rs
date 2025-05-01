mod derive_config;

#[proc_macro_derive(Config, attributes(config))]
pub fn derive_config(ts: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive_config::impl_(ts.into()).unwrap_or_else(syn::Error::into_compile_error).into()
}

mod derive_field_enum;

#[proc_macro_derive(FieldEnum, attributes(config, field_default))]
pub fn derive_field_enum(ts: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive_field_enum::impl_(ts.into()).unwrap_or_else(syn::Error::into_compile_error).into()
}
