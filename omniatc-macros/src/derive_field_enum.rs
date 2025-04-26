use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;

use crate::derive_config::{collect_attrs_docs, opts_expr};

pub fn impl_(input: TokenStream) -> syn::Result<TokenStream> {
    let input: syn::DeriveInput = syn::parse2(input)?;
    let ident = &input.ident;

    let discrim_ident = format_ident!("{ident}Discrim");

    let input_enum = match input.data {
        syn::Data::Struct(e) => {
            return Err(syn::Error::new_spanned(
                e.struct_token,
                "Only enums can derive(FieldEnum)",
            ));
        }
        syn::Data::Enum(e) => e,
        syn::Data::Union(e) => {
            return Err(syn::Error::new_spanned(e.union_token, "Only enums can derive(FieldEnum)"));
        }
    };

    let variant_idents: Vec<_> = input_enum.variants.iter().map(|variant| &variant.ident).collect();

    let mut default_value = None;

    let egui_arms = input_enum.variants.iter().map(|variant| {
        let variant_ident = &variant.ident;
        if let syn::Fields::Unnamed(..) = variant.fields {
            return Err(syn::Error::new_spanned(&variant.fields, "derive(FieldEnum) cannot be used on enums with tuple variants"));
        }

        let field_idents: Vec<_> = variant.fields.iter().map(|field| {
            field.ident.as_ref().expect("checked not unnamed")
        }).collect();
        let field_types: Vec<_> = variant.fields.iter().map(|field|  &field.ty ).collect();
        let field_defaults = variant.fields.iter().map(|field|  {
            let mut attrs = field.attrs.iter().filter_map(|attr| {
                match &attr.meta {
                    syn::Meta::List(meta) if meta.path.is_ident("field_default") => Some(&meta.tokens),
                    _ => None,
                }
            });
            attrs.next().ok_or_else(|| syn::Error::new_spanned(field, "every field must have a default value specified in `#[field_default(default_expr)]`"))
        }).collect::<syn::Result<Vec<_>>>()?;
        let field_docs: Vec<_> = variant.fields.iter().map(|field| collect_attrs_docs(&field.attrs)).collect();
        let field_opts = variant.fields.iter().map(|field| opts_expr(&field.attrs, &field.ty)).collect::<syn::Result<Vec<_>>>()?;

        if let Some(default_attr) = variant.attrs.iter().find(|attr| attr.path().is_ident("field_default")) {
            default_value = Some(quote_spanned! { default_attr.span() =>
                Self::#variant_ident {
                    #(#field_idents : #field_defaults,)*
                }
            });
        }

        Ok(quote_spanned! { variant.span() =>
            #discrim_ident::#variant_ident => {
                match self {
                    Self::#variant_ident { .. } => {},
                    _ => {
                        *self = Self::#variant_ident {
                            #(#field_idents: #field_defaults,)*
                        };
                        *ctx.changed = true;
                    },
                }

                let (__ui, __ctx, __meta) = (&mut *ui, &mut *ctx, &meta);

                let (#(#field_idents,)*) = match self {
                    Self::#variant_ident { #(#field_idents,)* } => (#(#field_idents,)*),
                    _ => unreachable!("just assigned"),
                };

                __ui.indent((&__meta.group, __meta.id, stringify!(#variant_ident)), |__ui| {
                    #(
                        <#field_types as crate::config::Field>::show_egui(
                            #field_idents,
                            crate::config::FieldMeta {
                                group: std::borrow::Cow::Owned(format!("{}.{}.{}", __meta.group, __meta.id, stringify!(#variant_ident))),
                                id: stringify!(#field_idents),
                                doc: #field_docs,
                                opts: #field_opts,
                            },
                            __ui,
                            __ctx,
                        );
                    )*
                });
            }
        })
    }).collect::<syn::Result<Vec<_>>>()?;

    let Some(default_value) = default_value else {
        return Err(syn::Error::new(
            Span::call_site(),
            "Exactly one FieldEnum variant should have the `#[field_default]` attribute",
        ));
    };

    let ts = quote! {
        #[allow(clippy::manual_let_else)]
        const _: () = {
            #[derive(Clone, Copy, PartialEq, Eq)]
            enum #discrim_ident {
                #(#variant_idents,)*
            }

            impl #discrim_ident {
                fn from(v: &#ident) -> Self {
                    match v {
                        #(#ident::#variant_idents { .. } => Self::#variant_idents,)*
                    }
                }

                fn ident(self) -> &'static str {
                    match self {
                        #(Self::#variant_idents => stringify!(#variant_idents),)*
                    }
                }
            }

            impl crate::config::Field for #ident {
                type Opts = ();

                fn show_egui(&mut self, meta: crate::config::FieldMeta<()>, ui: &mut bevy_egui::egui::Ui, ctx: &mut crate::config::FieldEguiContext) {
                    let mut selected_variant = #discrim_ident::from(&*self);

                    bevy_egui::egui::ComboBox::new(format!("{}.{}", meta.group, meta.id), meta.id)
                        .selected_text(format!("{}", selected_variant.ident()))
                        .show_ui(ui, |ui| {
                            #(
                                ui.selectable_value(
                                    &mut selected_variant,
                                    #discrim_ident::#variant_idents,
                                    format!("{}", #discrim_ident::#variant_idents.ident()),
                                );
                            )*
                        });

                    match selected_variant {
                        #(#egui_arms,)*
                    }
                }

                fn as_serialize(&self) -> impl serde::Serialize + '_ { self }

                type Deserialize = Self;
                fn from_deserialize(de: Self::Deserialize) -> Self { de }
            }

            impl Default for #ident {
                fn default() -> Self {
                    #default_value
                }
            }
        };
    };
    Ok(ts)
}
