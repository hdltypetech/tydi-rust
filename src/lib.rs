use std::fmt::{Debug, Display};
use bytemuck::{bytes_of, cast, cast_slice, from_bytes_mut, NoUninit, Pod};
use crate::binary::{bool_vec_to_tydi_binary, FromTydiBinary, ToTydiBinary, TydiBinary};

pub mod drilling;
pub mod binary;

#[derive(Debug)]
pub struct TydiStream<T>(pub Vec<TydiPacket<T>>);

#[derive(Debug)]
pub struct TydiBinaryStream(pub Vec<TydiBinary>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TydiPacket<T> {
    pub data: Option<T>,
    pub last: Vec<bool>,
}

impl<T> TydiPacket<T> {
    pub fn to_binary(self) -> TydiBinary where T: ToTydiBinary {
        let strobe: TydiBinary = self.data.is_some().into();
        let last_bin: TydiBinary = bool_vec_to_tydi_binary(self.last);
        // el.data.and_then(|data| { Some(data.into()) }).or(Some(TydiBinary { data: vec![], len: 0 }))
        let data_bin = if let Some(data) = self.data {
            let binary: TydiBinary = data.into();
            assert_eq!(binary.len, T::SIZE, "resulting binary not of expected size");
            binary
        } else {
            let n_bytes = T::SIZE.div_ceil(8);
            TydiBinary { data: vec![0u8; n_bytes], len: T::SIZE }
        };
        strobe.concatenate(&last_bin).concatenate(&data_bin)
    }

    pub fn from_binary(val: TydiBinary, dim: usize) -> Self where T: FromTydiBinary {
        let (strobe, res) = bool::from_tydi_binary(val);
        let (last, res) = res.split(dim);
        let last: Vec<bool> = last.into();

        let data: Option<T> = if strobe {
            let (item, _) = T::from_tydi_binary(res);
            Some(item)
        } else {
            None
        };
        Self { data, last }
    }

    pub fn map_data<B>(self, f: impl FnOnce(T) -> B) -> TydiPacket<B> {
        TydiPacket {
            data: self.data.and_then(|x| Some(f(x))),
            last: self.last,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TydiVec<T> {
    pub data: Vec<TydiPacket<T>>,
    d: i8,  // Dimensionality
}

impl<T> TydiVec<T> {
    pub fn new(d: i8) -> Self {
        TydiVec {
            data: Vec::new(),
            d,
        }
    }

    pub fn push(&mut self, data: Option<T>, last: Vec<bool>) {
        self.data.push(TydiPacket { data, last });
    }
}

impl From<&str> for TydiVec<u8> {
    /// Creates a TydiVec from a string.
    fn from(value: &str) -> Self {
        let bytes: &[u8] = value.as_bytes();
        let mut result: Vec<TydiPacket<u8>> = Vec::new();

        // Handle empty strings
        if bytes.is_empty() {
            return TydiVec {
                data: vec!(
                    TydiPacket {
                        data: None,
                        last: vec![true],  // Empty string marker
                    }
                ),
                d: 0
            }
        }

        for (i, &byte) in bytes.iter().enumerate() {
            let is_last_char = i == bytes.len() - 1;

            result.push(TydiPacket {
                data: Some(byte),
                last: vec![is_last_char],
            });
        }

        TydiVec {
            data: result,
            d: 0,
        }
    }
}

impl<T: Clone> From<Vec<T>> for TydiVec<T> {
    /// Creates a TydiVec from any vector.
    fn from(value: Vec<T>) -> Self {
        let mut result: Vec<TydiPacket<T>> = Vec::new();

        // Handle empty sequences
        if value.is_empty() {
            return TydiVec {
                data: vec!(
                    TydiPacket {
                        data: None,
                        last: vec![true],  // Empty sequence marker
                    }
                ),
                d: 0
            }
        }

        for (i, el) in value.iter().enumerate() {
            let is_last_el = i == value.len() - 1;

            result.push(TydiPacket {
                data: Some((*el).clone()),
                last: vec![is_last_el],
            });
        }

        TydiVec {
            data: result,
            d: 0
        }
    }
}

impl<T: Clone> From<Vec<TydiVec<T>>> for TydiVec<T> {
    /// Creates a TydiVec from any vector.
    fn from(value: Vec<TydiVec<T>>) -> Self {
        let mut result: Vec<TydiPacket<T>> = Vec::new();

        // Handle empty sequences
        if value.is_empty() {
            return TydiVec {
                data: vec!(
                    TydiPacket {
                        data: None,
                        last: vec![true, true],  // Fixme how do we know what dimension we should be at here?
                    }
                ),
                d: 0
            }
        }

        for (i, seq) in value.iter().enumerate() {
            let is_last_seq = i == value.len() - 1;

            for (j, el) in seq.data.iter().enumerate() {
                result.push(TydiPacket {
                    data: el.data.clone(),
                    last: [el.last.clone(), vec![is_last_seq]].concat(),
                });
            }
        }

        TydiVec {
            data: result,
            d: 0
        }
    }
}


#[cfg(test)]
mod tests {
    use crate::binary::TydiBinary;
    use super::*;

    #[test]
    fn test_struct_packing() {
        #[derive(Debug, PartialEq, Eq, Clone)]
        struct Comment {
            comment_id: u32,
            author: Author,
            content: String,
            created_at: String,
            likes: u32,
            in_reply_to_comment_id: Option<u32>,
        }

        #[derive(Debug, PartialEq, Eq, Clone)]
        struct Author {
            user_id: u32,
            username: String,
        }

        let data = Comment {
            comment_id: 1,
            author: Author {
                user_id: 789,
                username: "CultureVulture".into()
            },
            content: "Oh, Andalusia is truly magical! Did you get a chance to see any flamenco shows in Seville?".into(),
            created_at: "2025-06-15T12:05:00Z".into(),
            likes: 10,
            in_reply_to_comment_id: None,
        };

        impl From<Author> for TydiBinary {
            fn from(author: Author) -> TydiBinary {
                author.user_id.into()
            }
        }

        impl From<Comment> for TydiBinary {
            fn from(comment: Comment) -> TydiBinary {
                let binaries: Vec<TydiBinary> = vec![
                    comment.comment_id.into(),
                    comment.author.into(),
                    comment.likes.into(),
                ];
                binaries.iter().fold(TydiBinary::empty(), |acc, e| acc.concatenate(e)).clone()
            }
        }

        impl From<TydiBinary> for Comment {
            fn from(bin: TydiBinary) -> Self {
                let (comment_id, remainder) = bin.split_for();
                let (author_id, remainder) = remainder.split_for();
                let (likes, remainder) = remainder.split_for();

                Comment {
                    comment_id,
                    author: Author {
                        user_id: author_id,
                        username: "".to_string(),
                    },
                    content: "".to_string(),
                    created_at: "".to_string(),
                    likes,
                    in_reply_to_comment_id: None,
                }
            }
        }

        let le_value = 1u32.to_le_bytes();
        let be_value = 2u32.to_be_bytes();

        let bin = TydiBinary::from(data);
        println!("{}", bin);
        println!("{:?}", bin);
        let reconstructed: Comment = bin.into();
        println!("{:?}", reconstructed);
        println!("done");
    }

    #[test]
    fn test_packing() {
        let num_bytes: [u8; 8] = [0xed, 0x1, 0x0, 0x0, 0x20, 0x7, 0x0, 0x0];
        let num = u64::from_ne_bytes(num_bytes);
        let packet = TydiPacket {
            data: Some(num),
            last: vec![true],
        };
        let bin = packet.to_binary();
        let reconstructed: TydiPacket<u64> = TydiPacket::from_binary(bin, 1);
        assert_eq!(reconstructed.data, Some(num));
    }
}
