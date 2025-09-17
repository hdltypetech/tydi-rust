extern crate proc_macro;

mod tests;

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{parse_macro_input, parse2, ItemStruct, Data, DeriveInput, Fields, Ident, Type};

pub fn tydi_derive_impl(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    // let input = parse_macro_input!(input as DeriveInput);

    // proc_marco2 version of "parse_macro_input!(input as ItemFn)"
    let input = match parse2::<ItemStruct>(input) {
        Ok(syntax_tree) => syntax_tree,
        Err(error) => return error.to_compile_error(),
    };

    let struct_name = &input.ident; // e.g., User

    // Search for #[packed_size(N)]
    let mut packed_size: Option<usize> = None;
    for attr in &input.attrs {
        if attr.path().is_ident("packed_size") {
            let lit: syn::LitInt = attr.parse_args().unwrap();
            packed_size = Some(lit.base10_parse::<usize>().unwrap());
        }
    }

    let size = packed_size.expect("missing #[packed_size(N)] on struct");
    // This way you can get the number without type, but it's actually fine to have it with type.
    // let lit_size = syn::LitInt::new(&size.to_string(), Span::call_site());

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Collect fields for the non-Vec struct and Vec-only struct
    let mut non_vec_fields = Vec::new();
    let mut vec_fields = Vec::new();
    let mut all_fields = Vec::new();

    if let Fields::Named(fields_named) = &input.fields {
        for field in &fields_named.named {
            let field_name = field.ident.as_ref().expect("Expected named field");
            let field_type = &field.ty;

            // Check if the type is a Vec
            let is_vec = if let Type::Path(type_path) = field_type {
                let str_repr = type_path.path.segments.iter().map(|x| x.ident.to_string()).collect::<Vec<_>>().join(".");
                type_path.path.segments.last().map_or(false, |segment| {
                    segment.ident == "Vec" || segment.ident == "String" ||  segment.ident == "str"
                })
            } else {
                false
            };

            all_fields.push(field);
            if is_vec {
                vec_fields.push(field);
            } else {
                non_vec_fields.push(field);
            }
        }
    }

    // Now, we can generate the field definitions for the new structs
    let non_vec_fields_tokens = non_vec_fields.iter().map(|f| {
        let field_name = &f.ident;
        let field_type = &f.ty;
        quote! { pub #field_name: #field_type, }
    });

    let vec_fields_tokens = vec_fields.iter().map(|f| {
        let field_name = &f.ident;
        let field_type = &f.ty;
        quote! { pub #field_name: #field_type, }
    });

    /*// Generate the non-Vec struct
    let non_vec_struct_name = Ident::new(&format!("{}NonVecs", struct_name), struct_name.span());
    let generated_non_vec_struct = quote! {
        #[derive(Debug, PartialEq, Eq, Clone)] // Add common derives
        pub struct #non_vec_struct_name #ty_generics #where_clause {
            #(#non_vec_fields_tokens)*
        }
    };

    // Generate the Vec-only struct
    let vec_struct_name = Ident::new(&format!("{}Vecs", struct_name), struct_name.span());
    let generated_vec_struct = quote! {
        #[derive(Debug, PartialEq, Eq, Clone)] // Add common derives
        pub struct #vec_struct_name #ty_generics #where_clause {
            #(#vec_fields_tokens)*
        }
    };*/

    // Generate an impl From for original -> non_vec
    let to_tydi_binary_impl = {
        let field_conversions = non_vec_fields.iter().map(|f| {
            let field_name = &f.ident;
            quote! { let #field_name: TydiBinary = self.#field_name.into(); }
        });
        let concatenations = non_vec_fields.iter().map(|f| {
            let field_name = &f.ident;
            quote! { .concatenate(&#field_name) }
        });

        /*impl ToTydiBinary for Post {
            const SIZE: usize = 256;

            fn to_tydi_binary(self) -> TydiBinary {
                let post_id: TydiBinary = self.post_id.into();
                let author: TydiBinary = self.author.into();
                let created_at: TydiBinary = self.created_at.into();
                let updated_at: TydiBinary = self.updated_at.into();
                let likes: TydiBinary = self.likes.into();
                let shares: TydiBinary = self.shares.into();
                post_id.concatenate(&author).concatenate(&created_at).concatenate(&updated_at).concatenate(&likes).concatenate(&shares)
            }
        }*/

        quote! {
            impl #impl_generics ToTydiBinary for #struct_name #ty_generics #where_clause {
                const SIZE: usize = #size;

                fn to_tydi_binary(self) -> TydiBinary {
                    #(#field_conversions)*
                    TydiBinary::empty()#(#concatenations)*
                }
            }
        }
    };

    /*impl FromTydiBinary for Post {
        fn from_tydi_binary(value: TydiBinary) -> Self, TydiBinary) {
            let (post_id, res) = u32::from_tydi_binary(value);
            let (author, res) = Author::from_tydi_binary(res);
            let (created_at, res) = MyDate::from_tydi_binary(res);
            let (updated_at, res) = MyDate::from_tydi_binary(res);
            let (likes, res) = u32::from_tydi_binary(res);
            let (shares, res) = u32::from_tydi_binary(res);

            (Self {
                post_id,
                title: "".to_string(),
                content: "".to_string(),
                author,
                created_at,
                updated_at,
                tags: vec![],
                likes,
                shares,
                comments: vec![],
            }, res)
        }
    }*/

    // Generate an impl From for original -> vec
    let original_to_vec_impl = {
        let field_conversions = all_fields.iter().map(|f| {
            let field_name = &f.ident;
            let field_type = get_base_type_ident(&f.ty);
            quote! { let (#field_name, res) = #field_type::from_tydi_binary(res); }
        });
        let field_names = all_fields.iter().map(|f| {
            &f.ident
        });

        quote! {
            impl #impl_generics FromTydiBinary for #struct_name #ty_generics #where_clause {
                fn from_tydi_binary(value: TydiBinary) -> (Self, TydiBinary) {
                    let res = value;
                    #(#field_conversions)*

                    (Self {
                        #(#field_names),*
                    }, res)
                }
            }
        }
    };


    // Combine all generated tokens
    let expanded = quote! {
        #to_tydi_binary_impl
        #original_to_vec_impl
    };

    expanded.into()
}

fn get_base_type_ident(ty: &Type) -> Option<&Ident> {
    if let Type::Path(syn::TypePath { path, .. }) = ty {
        // Get the last segment of the path
        if let Some(segment) = path.segments.last() {
            // Return the identifier of that segment
            return Some(&segment.ident);
        }
    }
    None
}
