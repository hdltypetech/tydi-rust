use serde::{Deserialize, Serialize};
use std::fs;
use std::error::Error;
use chrono::{DateTime, Utc};
use rust_tydi_packages::{binary::TydiBinary, TydiPacket, drilling::*, TydiStream, TydiBinaryStream};
use rust_tydi_packages::binary::{FromTydiBinary, ToTydiBinary};
// Define the data structures based on the JSON schema.
// We use `serde::Deserialize` to automatically derive the deserialization logic.

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
struct MyDate(DateTime<Utc>);

// Represents a single comment.
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
struct Comment {
    comment_id: u32,
    author: Author,
    content: String,
    created_at: MyDate,
    likes: u32,
    // The `in_reply_to_comment_id` field is optional, so we use `Option<u32>`.
    in_reply_to_comment_id: Option<u32>,
}

// Represents the author of a post or comment.
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
struct Author {
    user_id: u32,
    username: String,
}

// Represents a single post.
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
struct Post {
    post_id: u32,
    title: String,
    content: String,
    author: Author,
    created_at: MyDate,
    updated_at: MyDate,
    tags: Vec<String>,
    likes: u32,
    shares: u32,
    comments: Vec<Comment>,
}

impl From<MyDate> for TydiBinary {
    fn from(value: MyDate) -> Self {
        let temp: u64 = value.0.timestamp_millis() as u64;
        temp.into()
    }
}

impl FromTydiBinary for MyDate {
    fn from_tydi_binary(value: TydiBinary) -> (Self, TydiBinary) {
        let (int_value, res) = i64::from_tydi_binary(value);
        let dt = DateTime::from_timestamp_millis(int_value).unwrap();
        (MyDate(dt), res)
    }
}

// The root data structure, which is a vector of posts.
#[derive(Debug, Deserialize)]
struct Posts(Vec<Post>);

struct MyTypeStreams {
    subStream: Vec<TydiBinary>,
    subStream2: Vec<TydiBinary>
}

struct MyTypeProcessed {
    someProp: TydiBinary, // from bool
    someOtherProp: TydiBinary, // from u8

    streams: MyTypeStreams,
}

struct PhysicalStreamsTyped {
    posts: TydiStream<Post>,
    post_titles: TydiStream<u8>,
    post_contents: TydiStream<u8>,
    post_author_username: TydiStream<u8>,
    post_tags: TydiStream<u8>,
    post_comments: TydiStream<Comment>,
    post_comment_author_username: TydiStream<u8>,
    post_comment_content: TydiStream<u8>,
}

impl PhysicalStreamsTyped {
    pub fn new(posts: Vec<Post>) -> PhysicalStreamsTyped {
        let posts_tydi = posts.convert();
        let titles_tydi = posts_tydi.drill(|e| e.title.as_bytes().to_vec());
        let contents_tydi = posts_tydi.drill(|e| e.content.as_bytes().to_vec());
        let tags_tydi = posts_tydi.drill(|e| e.tags.clone()).drill(|e| e.as_bytes().to_vec());
        let comments_tydi = posts_tydi.drill(|e| e.comments.clone());
        let comment_author_tydi = comments_tydi.drill(|e| e.author.username.as_bytes().to_vec());

        PhysicalStreamsTyped {
            post_titles: titles_tydi,
            post_contents: contents_tydi,
            post_author_username: posts_tydi.drill(|e| e.author.username.as_bytes().to_vec()),
            post_tags: tags_tydi,
            post_comment_author_username: comment_author_tydi,
            post_comment_content: comments_tydi.drill(|e| e.content.as_bytes().to_vec()),
            post_comments: comments_tydi,
            posts: posts_tydi,
        }
    }

    pub fn reverse(self) -> Vec<Post> {
        let mut comments_recreated = self.post_comments;
        comments_recreated.inject_string(|el| &mut el.author.username, self.post_comment_author_username);
        comments_recreated.inject_string(|el| &mut el.content, self.post_comment_content);

        let mut posts_recreated = self.posts;
        posts_recreated.inject(|el| &mut el.comments, comments_recreated);
        let tags_recreated = self.post_tags.solidify_into_strings();
        posts_recreated.inject(|el| &mut el.tags, tags_recreated);
        posts_recreated.inject_string(|el| &mut el.title, self.post_titles);
        posts_recreated.inject_string(|el| &mut el.content, self.post_contents);
        posts_recreated.inject_string(|el| &mut el.author.username, self.post_author_username);

        posts_recreated.unpack()
    }
}

struct PhysicalStreamsBinary {
    posts: TydiBinaryStream,
    post_titles: TydiBinaryStream,
    post_contents: TydiBinaryStream,
    post_author_username: TydiBinaryStream,
    post_tags: TydiBinaryStream,
    post_comments: TydiBinaryStream,
    post_comment_author_username: TydiBinaryStream,
    post_comment_content: TydiBinaryStream,
}

impl PhysicalStreamsBinary {
    pub fn new(posts: PhysicalStreamsTyped) -> PhysicalStreamsBinary {
        Self {
            posts: posts.posts.finish(),
            post_titles: posts.post_titles.finish(),
            post_contents: posts.post_contents.finish(),
            post_author_username: posts.post_author_username.finish(),
            post_tags: posts.post_tags.finish(),
            post_comments: posts.post_comments.finish(),
            post_comment_author_username: posts.post_comment_author_username.finish(),
            post_comment_content: posts.post_comment_content.finish(),
        }
    }

    pub fn reverse(self) -> PhysicalStreamsTyped {
        PhysicalStreamsTyped {
            posts: packets_from_binaries(self.posts, 1),
            post_titles: packets_from_binaries(self.post_titles, 2),
            post_contents: packets_from_binaries(self.post_contents, 2),
            post_author_username: packets_from_binaries(self.post_author_username, 2),
            post_tags: packets_from_binaries(self.post_tags, 3),
            post_comments: packets_from_binaries(self.post_comments, 2),
            post_comment_author_username: packets_from_binaries(self.post_comment_author_username, 3),
            post_comment_content: packets_from_binaries(self.post_comment_content, 3),
        }

    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // This assumes the JSON file is named 'posts.json' and is in the same directory.
    let json_file_path = "posts.json";
    let recreation_file_path = "posts-rec.json";

    // Read the contents of the JSON file into a string.
    let json_data = fs::read_to_string(json_file_path)
        .expect("Should have been able to read the file");

    // Deserialize the JSON string into our `Posts` data structure.
    let posts: Vec<Post> = serde_json::from_str(&json_data)?;

    // Print original data summary
    println!("=== Original Data Summary ===");
    for post in &posts {
        println!("Title: {}", post.title);
        println!("Author: {}", post.author.username);
        println!("Likes: {}", post.likes);
        println!("Tags: {:?}", post.tags);
        println!("Number of Comments: {}\n", post.comments.len());
    }

    let typed_streams = PhysicalStreamsTyped::new(posts.clone());
    let binary_streams = PhysicalStreamsBinary::new(typed_streams);

    println!("author stream binary: {:?}", binary_streams.post_comment_author_username.0.iter().map(|e| e.to_string()).collect::<Vec<String>>());
    println!("author stream native: {:?}", posts.iter().flat_map(|e| e.comments.clone()).flat_map(|e| e.author.username.as_bytes().iter().map(|e| format!("{:08b}", e)).collect::<Vec<_>>()).collect::<Vec<_>>());

    println!("posts binary: {:?}", binary_streams.posts);
    let objs = binary_streams.reverse();
    let reconstructed_posts = objs.reverse();
    // comments_recreated.inject(|e| e.author.username, comment_author_recreated);
    // posts_recreated[0].data.unwrap().comments.push()
    let my_var = 5;

    let json_recreated = serde_json::to_string(&reconstructed_posts).expect("Should have been able to serialize the reconstructed posts");
    fs::write(recreation_file_path, json_recreated);

    Ok(())
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PostNonVecs {
    pub post_id: u32,
    pub author: AuthorNonVecs,
    pub created_at: MyDate,
    pub updated_at: MyDate,
    pub likes: u32,
    pub shares: u32,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PostVecs {
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub comments: Vec<Comment>,
}

impl ToTydiBinary for Post {
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
}

impl From<TydiBinary> for Post {
    fn from(value: TydiBinary) -> Self {
        let (post_id, res) = u32::from_tydi_binary(value);
        let (author, res) = Author::from_tydi_binary(res);
        let (created_at, res) = MyDate::from_tydi_binary(res);
        let (updated_at, res) = MyDate::from_tydi_binary(res);
        let (likes, res) = u32::from_tydi_binary(res);
        let (shares, res) = u32::from_tydi_binary(res);

        Self {
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
        }
    }
}

impl FromTydiBinary for Post {
    fn from_tydi_binary(value: TydiBinary) -> (Self, TydiBinary) {
        (value.into(), TydiBinary::empty())
    }
}

impl From<Author> for TydiBinary {
    fn from(value: Author) -> Self {
        let author_id: TydiBinary = value.user_id.into();
        author_id
    }
}

impl FromTydiBinary for Author {
    fn from_tydi_binary(value: TydiBinary) -> (Self, TydiBinary) {
        let (user_id, res) = u32::from_tydi_binary(value);
        let author = Self {
            user_id,
            username: "".to_string(),
        };
        (author, res)
    }
}

impl FromTydiBinary for Comment {
    fn from_tydi_binary(value: TydiBinary) -> (Self, TydiBinary) {
        let (comment_id, res) = u32::from_tydi_binary(value);
        let (author, res) = Author::from_tydi_binary(res);
        let (created_at, res) = MyDate::from_tydi_binary(res);
        let (likes, res) = u32::from_tydi_binary(res);
        let author = Self {
            comment_id,
            author,
            content: "".to_string(),
            created_at,
            likes,
            in_reply_to_comment_id: None,
        };
        (author, res)
    }
}

impl ToTydiBinary for Comment {
    const SIZE: usize = 160;

    fn to_tydi_binary(self) -> TydiBinary {
        let comment_id: TydiBinary = self.comment_id.into();
        let author: TydiBinary = self.author.into();
        let created_at: TydiBinary = self.created_at.into();
        let likes: TydiBinary = self.likes.into();
        comment_id.concatenate(&author).concatenate(&created_at).concatenate(&likes)
    }
}

impl From<Post> for PostNonVecs { fn from(value: Post) -> Self { Self { post_id: value.post_id, author: value.author.into(), created_at: value.created_at, updated_at: value.updated_at, likes: value.likes, shares: value.shares } } }

impl From<Post> for PostVecs { fn from(value: Post) -> Self { Self { title: value.title, content: value.content, tags: value.tags, comments: value.comments } } }


#[derive(Debug, PartialEq, Eq, Clone)]
pub struct AuthorNonVecs {
    pub user_id: u32,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct AuthorVecs {
    pub username: String,
}

impl From<Author> for AuthorNonVecs { fn from(value: Author) -> Self { Self { user_id: value.user_id } } }

impl From<Author> for AuthorVecs { fn from(value: Author) -> Self { Self { username: value.username } } }

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CommentNonVecs {
    pub comment_id: u32,
    pub author: Author,
    pub created_at: MyDate,
    pub likes: u32,
    pub in_reply_to_comment_id: Option<u32>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CommentVecs {
    pub content: String,
}

impl From<Comment> for CommentNonVecs { fn from(value: Comment) -> Self { Self { comment_id: value.comment_id, author: value.author, created_at: value.created_at, likes: value.likes, in_reply_to_comment_id: value.in_reply_to_comment_id } } }

impl From<Comment> for CommentVecs { fn from(value: Comment) -> Self { Self { content: value.content } } }

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Timelike, Utc};
    use rust_tydi_packages::binary::{FromTydiBinary, TydiBinary};
    use crate::MyDate;

    #[test]
    fn test_date_time_packing() {
        let dt_original = Utc::now().with_nanosecond(0).unwrap();
        let dt = MyDate(dt_original);
        let binary: TydiBinary = dt.clone().into();
        let (reconstructed, _) = MyDate::from_tydi_binary(binary);
        assert_eq!(reconstructed, dt);
    }
}
