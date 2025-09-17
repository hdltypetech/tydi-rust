extern crate proc_macro;

mod tests;

use proc_macro2::TokenStream;
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

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Collect fields for the non-Vec struct and Vec-only struct
    let mut non_vec_fields = Vec::new();
    let mut vec_fields = Vec::new();

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

    // Generate the non-Vec struct
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
    };

    // Generate an impl From for original -> non_vec
    let original_to_non_vec_impl = {
        let field_assignments = non_vec_fields.iter().map(|f| {
            let field_name = &f.ident;
            quote! { #field_name: value.#field_name }
        });

        quote! {
        impl #impl_generics From<#struct_name #ty_generics> for #non_vec_struct_name #ty_generics #where_clause {
            fn from(value: #struct_name #ty_generics) -> Self {
                    Self {
                        #(#field_assignments),*
                    }
                }
            }
        }
    };

    // Generate an impl From for original -> vec
    let original_to_vec_impl = {
        let field_assignments = vec_fields.iter().map(|f| {
            let field_name = &f.ident;
            quote! { #field_name: value.#field_name }
        });

        quote! {
            impl #impl_generics From<#struct_name #ty_generics> for #vec_struct_name #ty_generics #where_clause {
                fn from(value: #struct_name #ty_generics) -> Self {
                    Self {
                        #(#field_assignments),*
                    }
                }
            }
        }
    };


    // Combine all generated tokens
    let expanded = quote! {
        #generated_non_vec_struct
        #generated_vec_struct
        #original_to_non_vec_impl
        #original_to_vec_impl
    };

    expanded.into()
}
