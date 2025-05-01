use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;

pub fn impl_(input: TokenStream) -> syn::Result<TokenStream> {
    let input: syn::DeriveInput = syn::parse2(input)?;
    let ident = &input.ident;

    let input_struct = match input.data {
        syn::Data::Struct(ref v) => v,
        syn::Data::Enum(e) => {
            return Err(syn::Error::new_spanned(e.enum_token, "Only structs can derive(Config)"));
        }
        syn::Data::Union(e) => {
            return Err(syn::Error::new_spanned(e.union_token, "Only structs can derive(Config)"));
        }
    };

    let Some((attr_span, attr_args)) = input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("config"))
        .map(|attr| (attr.pound_token.span, attr.parse_args::<AttrArgs>()))
        .next()
    else {
        return Err(syn::Error::new_spanned(
            input_struct.struct_token,
            "Missing #[config] attribute",
        ));
    };
    let attr_args = attr_args?;

    let Some(group_id) = attr_args.id else {
        return Err(syn::Error::new(attr_span, "Missing config `id`"));
    };
    let Some(group_name) = attr_args.name else {
        return Err(syn::Error::new(attr_span, "Missing config `name`"));
    };

    let visit_fields = visit_fields(&input_struct.fields, &group_id)?;

    let out = quote! {
        impl crate::config::Config for #ident {
            fn save_id() -> &'static str { #group_id }
            fn name() -> &'static str { #group_name }

            fn for_each_field<V: crate::config::FieldVisitor>(&mut self, visitor: &mut V, ctx: &mut crate::config::FieldEguiContext) {
                #(#visit_fields)*
            }
        }
    };
    Ok(out)
}

fn visit_fields(fields: &syn::Fields, group_id: &str) -> syn::Result<Vec<TokenStream>> {
    fields
        .iter()
        .map(|field| {
            let Some(field_ident) = &field.ident else {
                return Err(syn::Error::new_spanned(
                    fields,
                    "Only structs with named fields can derive(Config)",
                ));
            };
            let field_ident_string = field_ident.to_string();

            let doc = collect_attrs_docs(&field.attrs);

            let opts = opts_expr(&field.attrs, &field.ty)?;

            Ok(quote! {
                visitor.visit_field(crate::config::FieldMeta {
                    group: #group_id.into(),
                    id: #field_ident_string,
                    doc: #doc,
                    opts: #opts,
                }, &mut self.#field_ident, ctx);
            })
        })
        .collect()
}

pub(crate) fn opts_expr(
    attrs: &[syn::Attribute],
    field_type: &syn::Type,
) -> syn::Result<TokenStream> {
    let opts: Vec<_> = attrs
        .iter()
        .filter_map(|attr| match &attr.meta {
            syn::Meta::List(opts) if opts.path.is_ident("config") => Some(&opts.tokens),
            _ => None,
        })
        .map(|ts| syn::parse2::<ConfigOpts>(ts.clone()))
        .collect::<syn::Result<Vec<_>>>()?;
    let update_opt_stmts = opts.iter().flat_map(|opts| &opts.0).map(
        |ConfigOpt { path, eq, expr }| quote_spanned!(eq.span => opts.#path = Into::into(#expr);),
    );

    Ok(quote! {{
        let mut opts = <#field_type as crate::config::Field>::Opts::default();
        #(#update_opt_stmts)*
        opts
    }})
}

pub(crate) fn collect_attrs_docs(attrs: &[syn::Attribute]) -> String {
    attrs
        .iter()
        .filter_map(|attr| match &attr.meta {
            syn::Meta::NameValue(syn::MetaNameValue {
                path,
                value: syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(lit), .. }),
                ..
            }) if path.is_ident("doc") => {
                let value = lit.value();
                if value.is_empty() { Some(String::from("\n")) } else { Some(lit.value()) }
            }
            _ => None,
        })
        .collect()
}

struct AttrArgs {
    id:   Option<String>,
    name: Option<String>,
}

impl Parse for AttrArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut out = AttrArgs { id: None, name: None };

        while !input.is_empty() {
            let lh = input.lookahead1();
            if lh.peek(kw::id) {
                input.parse::<kw::id>().unwrap();
                input.parse::<syn::Token![=]>()?;
                out.id = Some(input.parse::<syn::LitStr>()?.value());
            } else if lh.peek(kw::name) {
                input.parse::<kw::name>().unwrap();
                input.parse::<syn::Token![=]>()?;
                out.name = Some(input.parse::<syn::LitStr>()?.value());
            } else {
                return Err(lh.error());
            }

            if input.is_empty() {
                break;
            }

            input.parse::<syn::Token![,]>()?;
        }

        Ok(out)
    }
}

pub(crate) struct ConfigOpts(Punctuated<ConfigOpt, syn::Token![,]>);

impl Parse for ConfigOpts {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Punctuated::parse_terminated(input).map(Self)
    }
}

pub(crate) struct ConfigOpt {
    path: Punctuated<syn::Ident, syn::Token![.]>,
    eq:   syn::Token![=],
    expr: syn::Expr,
}

impl Parse for ConfigOpt {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let path = Punctuated::parse_separated_nonempty(input)?;
        if !input.peek(syn::Token![=]) {
            return Ok(Self {
                eq: syn::Token![=](path.span()),
                expr: syn::parse_quote_spanned!(path.span() => true),
                path,
            });
        }

        let eq = input.parse()?;
        let expr: syn::Expr = input.parse()?;

        Ok(Self { path, eq, expr: syn::parse_quote_spanned!(expr.span() => Some(#expr)) })
    }
}

mod kw {
    syn::custom_keyword!(id);
    syn::custom_keyword!(name);
}
