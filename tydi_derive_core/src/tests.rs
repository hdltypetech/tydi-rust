#![cfg(test)]

use crate::{tydi_derive_impl};
use proc_macro2::TokenStream;
use quote::quote;

#[test]
fn first() {
    let input = quote! {
        struct TestPost {
            post_id: u32,
            title: String,
            content: String,
            author: TestAuthor,
            created_at: String,
            updated_at: String,
            tags: Vec<String>,
            likes: u32,
            shares: u32,
            comments: Vec<TestComment>,
        }
    };

    let after = tydi_derive_impl(input);
    let after_str = after.to_string();
    println!("{}", after_str);
    println!("done");
}

#[test]
fn comment() {
    let input = quote! {
        #[packed_size(169)]
        struct Comment {
            comment_id: u32,
            author: Author,
            content: String,
            created_at: MyDate,
            likes: u32,
            // The `in_reply_to_comment_id` field is optional, so we use `Option<u32>`.
            in_reply_to_comment_id: Option<u32>,
        }
    };

    let after = tydi_derive_impl(input);
    let after_str = after.to_string();
    println!("{}", after_str);
    println!("done");
}

#[test]
fn author() {
    let input = quote! {
        #[packed_size(8)]
        struct Author {
            user_id: u32,
            username: String,
        }
    };

    let after = tydi_derive_impl(input);
    let after_str = after.to_string();
    println!("{}", after_str);
    println!("done");
}
