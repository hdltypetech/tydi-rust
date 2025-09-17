use std::mem;
use std::fmt;
use std::fmt::{Debug, Display};
use bytemuck::Pod;

#[derive(Clone, PartialEq, Eq)]
pub struct TydiBinary {
    pub data: Vec<u8>,
    pub len: usize,
}


impl TydiBinary {
    pub fn empty() -> Self {
        Self { data: Vec::new(), len: 0 }
    }

    /// Creates a new TydiBinary struct from a vector of bytes and a bit length.
    pub fn new(data: Vec<u8>, len: usize) -> Self {
        // Simple sanity check to ensure the length is not greater than
        // the capacity of the data vector.
        assert!(len <= data.len() * 8, "Length cannot exceed data capacity");
        Self { data, len }
    }

    /// Concatenates this TydiBinary with another one, returning a new TydiBinary.
    pub fn concatenate(&self, other: &Self) -> Self {
        // If this TydiBinary is empty, the result is simply a clone of the other.
        if self.len == 0 {
            return other.clone();
        }

        // Calculate the total length of the new binary string.
        let new_len = self.len + other.len;

        // Calculate the number of bits already in the last byte of `self`.
        let self_tail_bits = self.len % 8;

        // If `self` is byte-aligned, we can simply extend its data with `other`'s data.
        if self_tail_bits == 0 {
            let mut new_data = self.data.clone();
            new_data.extend_from_slice(&other.data);
            return Self::new(new_data, new_len);
        }

        // The number of bits needed to complete the last byte of `self`.
        let tail_space = 8 - self_tail_bits;

        // Clone the data from the first binary to start building the new vector.
        let mut new_data = self.data.clone();

        // Handle the last byte of `self` and its combination with the first bytes of `other`.
        // This is the core of the non-byte-aligned concatenation.
        for (i, &other_byte) in other.data.iter().enumerate() {
            // Get a mutable reference to the last byte of `new_data`.
            let last_byte = new_data.last_mut().unwrap();

            // Fill the remaining space in the last byte of `self` with bits from `other_byte`.
            let bits_from_other = other_byte << (8 - tail_space);
            *last_byte |= bits_from_other;

            // If we're not at the end of the `other` data, push the carry-over bits
            // as a new byte. The carry-over bits are the lower `tail_space` bits
            // of the current `other` byte, shifted into a new byte.
            if i < other.data.len() - 1 || (other.len - (i * 8) > tail_space) {
                let carry_over = (other_byte >> tail_space);
                new_data.push(carry_over);
            }
        }

        Self::new(new_data, new_len)
    }

    /// Splits this TydiBinary into two new TydiBinary instances at the specified length.
    /// Returns a tuple of (TydiBinary, TydiBinary).
    pub fn split(&self, len1: usize) -> (Self, Self) {
        let len2 = self.len - len1;

        // Part 1: First TydiBinary
        let mut data1 = Vec::new();
        let full_bytes1 = len1 / 8;
        let bit_offset = len1 % 8;
        for i in 0..full_bytes1 {
            data1.push(self.data[i]);
        }
        // Add leftover bits that are not a full byte
        if bit_offset > 0 {
            let byte_to_push = self.data[full_bytes1] & (!0u8 >> (8 - bit_offset));
            data1.push(byte_to_push);
        }
        let bin1 = TydiBinary::new(data1, len1);

        // Part 2: Second TydiBinary
        let mut data2 = Vec::new();
        let full_bytes2 = len2 / 8;
        let rem_bits2 = len2 % 8;

        let bytes_to_add = full_bytes2 + if rem_bits2 > 0 { 1 } else { 0 };
        // If the first binary ended at a clean border we need to subtract one to get the index of the last accessed byte
        let start_index = full_bytes1 - if bit_offset > 0 { 0 } else { 1 };
        for i in start_index..(start_index+bytes_to_add) {
            let next_byte_index = i + 1;

            let current_byte_original = self.data[i];
            let next_byte_original = if next_byte_index < self.data.len() {
                self.data[next_byte_index]
            } else {
                0
            };

            let new_byte = if (bit_offset == 0) {
                next_byte_original
            } else {
                (current_byte_original >> bit_offset) | (next_byte_original << (8 - bit_offset))
            };
            data2.push(new_byte);
        }

        let bin2 = TydiBinary::new(data2, len2);
        (bin1, bin2)
    }

    pub fn split_for<T: Pod>(&self) -> (T, TydiBinary) {
        let len = size_of::<T>() * 8;
        let (split1, split2) = self.split(len);
        let val: T = *bytemuck::from_bytes(split1.data.as_slice());
        (val, split2)
    }
}

impl Display for TydiBinary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Handle empty binary string
        if self.len == 0 {
            return write!(f, "0b");
        }

        // Calculate the number of full bytes and remaining bits
        let full_bytes = self.len / 8;
        let remaining_bits = self.len % 8;

        // Start with the "0b" prefix
        write!(f, "0b")?;

        // We print from last byte to first byte because of the little endian memory layout

        // Format the last byte with the remaining bits
        if remaining_bits > 0 {
            // We need to mask the data to get rid of the leading zeros. We do this with all 1's shifted to the right to zero-out any bits outside our length.
            let mask = 0xFF >> (8-remaining_bits);
            let last_byte = self.data[full_bytes];
            let masked_bits = last_byte & mask;
            write!(f, "{:0w$b}", masked_bits, w = remaining_bits)?;
        }

        // Format the full bytes
        for i in (0..full_bytes).rev() {
            write!(f, "{:08b}", self.data[i])?;
        }

        Ok(())
    }
}

impl Debug for TydiBinary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Create formatted strings for binary and hexadecimal representations
        let mut binary_string = String::new();
        let mut hex_string = String::new();

        // Handle the case where len is 0 to avoid panicking on an empty vector.
        if self.len > 0 {
            // Full bytes
            let full_bytes = self.len / 8;
            let remaining_bits = self.len % 8;

            // We print from last byte to first byte because of the little endian memory layout

            // Process the last, potentially partial, byte
            if remaining_bits > 0 {
                // The number of bits to extract from the last byte is `remaining_bits`.
                // We need to mask the data to get rid of the leading zeros. We do this with all 1's shifted to the right to zero-out any bits outside our length.
                let mask = 0xFF >> (8-remaining_bits);
                let last_byte = self.data[full_bytes];
                let masked_bits = last_byte & mask;

                // For the hexadecimal representation, we just take the full byte
                // because the hexadecimal string represents the underlying `Vec<u8>`.
                binary_string.push_str(&format!("{:0w$b}", masked_bits, w = remaining_bits));
                hex_string.push_str(&format!("{:02x}", self.data[full_bytes]));
            }

            // Process full bytes
            for i in (0..full_bytes).rev() {
                binary_string.push_str(&format!("{:08b} ", self.data[i]));
                hex_string.push_str(&format!("{:02x} ", self.data[i]));
            }
        }

        // Build the Debug struct representation
        f.debug_struct("TydiBinary")
            .field("len", &self.len)
            .field("data", &self.data)
            .field("binary", &binary_string.trim())
            .field("hex", &hex_string.trim())
            .finish()
    }
}

macro_rules! impl_from_primitive {
    ($($t:ty),*) => {
        $(
            impl ToTydiBinary for $t {
                const SIZE: usize = mem::size_of::<$t>() * 8;

                fn to_tydi_binary(self) -> TydiBinary {
                    TydiBinary {
                        data: self.to_ne_bytes().to_vec(),
                        len: Self::SIZE,
                    }
                }
            }

            impl From<TydiBinary> for $t {
                fn from(value: TydiBinary) -> Self {
                    let (int_bytes, _) = value.data.split_at(size_of::<$t>());
                    <$t>::from_ne_bytes(int_bytes.try_into().unwrap())
                }
            }

            impl FromTydiBinary for $t {
                fn from_tydi_binary(value: TydiBinary) -> (Self, TydiBinary) {
                    let (bin1, bin2) = value.split(size_of::<$t>() * 8);
                    let int_bytes = bin1.data.as_slice();
                    (<$t>::from_ne_bytes(int_bytes.try_into().unwrap()), bin2)
                }
            }
        )*
    };
}

impl_from_primitive!(u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64);

pub trait ToTydiBinary where Self: Sized {
    /// Number of bits this type always occupies when converted to binary.
    const SIZE: usize;

    /// Converts/packs the current datatype into a Tydi binary blob.
    fn to_tydi_binary(self) -> TydiBinary;
}

impl<T> From<T> for TydiBinary where T : ToTydiBinary {
    fn from(value: T) -> Self {
        value.to_tydi_binary()
    }
}

pub trait FromTydiBinary where Self: Sized {
    fn from_tydi_binary(value: TydiBinary) -> (Self, TydiBinary);
}

impl From<char> for TydiBinary {
    fn from(value: char) -> Self {
        let val: Vec<u8> = value.to_string().as_bytes().into();
        Self { data: val, len: 8*4 }
    }
}

impl From<bool> for TydiBinary {
    fn from(value: bool) -> Self {
        Self { data: vec![value.into()], len: 1 }
    }
}

impl FromTydiBinary for bool {
    fn from_tydi_binary(value: TydiBinary) -> (Self, TydiBinary) {
        let (bin1, bin2) = value.split(1);
        let b_value: bool = bin1.data[0] != 0;
        (b_value, bin2)
    }
}

impl From<Vec<bool>> for TydiBinary {
    fn from(value: Vec<bool>) -> Self {
        let bit_count = value.len();
        let byte_count = bit_count.div_ceil(8);

        let mut packed_bytes = Vec::with_capacity(byte_count);

        // Iterate over the boolean vector in chunks of 8.
        for chunk in value.chunks(8) {
            let mut byte: u8 = 0;
            let mut bit_position: u8 = 0;

            // Iterate through each boolean in the current chunk.
            for &value in chunk {
                if value {
                    // If the boolean is `true`, set the corresponding bit in the byte.
                    // We use a bitwise OR (`|=`) and left-shift a `1` to the
                    // correct position. `1 << bit_position` creates a byte with
                    // only a single bit set at the correct index.
                    byte |= 1 << bit_position;
                }
                // Move to the next bit position.
                bit_position += 1;
            }

            // Push the completed byte to the result vector.
            packed_bytes.push(byte);
        }
        TydiBinary { data: packed_bytes, len: bit_count }
    }
}

impl<T> FromTydiBinary for Vec<T> where T: FromTydiBinary {
    fn from_tydi_binary(value: TydiBinary) -> (Self, TydiBinary) {
        let (val, bin2) = T::from_tydi_binary(value);
        todo!();
    }
}

impl From<TydiBinary> for Vec<bool> {
    fn from(value: TydiBinary) -> Self {
        let packed_bytes = &value.data;
        let bit_count = value.len;

        // Pre-allocate the vector with the exact size for efficiency.
        let mut bools = Vec::with_capacity(bit_count);

        // Iterate through each packed byte.
        for &byte in packed_bytes {
            // Iterate through each of the 8 bits in the byte.
            for i in 0..8 {
                // Check if the current bit is set using a bitwise AND operation.
                // (byte & (1 << i)) creates a value with only the i-th bit set if it was
                // set in the original byte. Comparing this to 0 checks if it was a 1.
                if (byte & (1 << i)) != 0 {
                    bools.push(true);
                } else {
                    bools.push(false);
                }
            }
        }

        // Truncate the vector to the original boolean count. This is important
        // to handle the case where the last byte was padded with zeros.
        bools.truncate(bit_count);

        bools
    }
}

#[cfg(test)]
mod tests {
    use crate::binary::TydiBinary;

    #[test]
    fn test_binary_glue() {
        let bin1 = TydiBinary {
            data: vec![0b10101010, 0b11110000],
            len: 16,
        };
        let string1 = bin1.to_string();
        assert_eq!(string1, "0b1111000010101010");

        let bin2 = TydiBinary {
            data: vec![0b10101010, 0b00001111], // 0xaa, 0x0f
            len: 12,
        };
        let string2 = bin2.to_string();
        assert_eq!(string2, "0b111110101010");

        let last_bin = TydiBinary { data: vec![0b101], len: 3 }; // Value = 5
        let char_bin = TydiBinary { data: vec![0b01000011], len: 8 }; // Value = 67 or 0x43
        let package = last_bin.concatenate(&char_bin); // Expected value = 0x021D or [0x1D, 0x02]
        assert_eq!(package.data[0], 0x1D);
        assert_eq!(package.data[1], 0x02);
        let package_string = package.to_string();
        assert_eq!(package_string, "0b01000011101");

        let bin3 = TydiBinary::new(vec![0xAB, 0x0C], 12);
        // 0xAB = 1010 1011, 0xC0 = 1100 0000
        let string3 = bin3.to_string();
        assert_eq!(string3, "0b110010101011");

        let bin4 = TydiBinary::new(vec![0xDE, 0x0F], 16);
        // 0xDE = 1101 1110, 0x0F = 0000 1111
        let string4 = bin4.to_string();
        assert_eq!(string4, "0b0000111111011110");

        let result2 = bin3.concatenate(&bin4);
        let result_string2 = result2.to_string();
        assert_eq!(result_string2, "0b0000111111011110110010101011");
        let (recovered3, recovered4) = result2.split(12);
        println!("recovered3: {:?} (recovered4: {:?})\n", recovered3, recovered4);
        assert_eq!(recovered3.data, vec![0xAB, 0x0C]);
        assert_eq!(recovered3.to_string(), "0b110010101011");
        assert_eq!(recovered4.data, vec![0xDE, 0x0F]);
        assert_eq!(recovered4.to_string(), "0b0000111111011110");

        let number = 123456789u64;
        let tydi_number: TydiBinary = number.into();
        println!("number: {}, tydi: {:?}", number, tydi_number);
    }

    #[test]
    fn test_binary_from_u32() {
        // let value: u32 = 0x12345678;
        let value = 12345678u64;
        let binary = TydiBinary::from(value);

        assert_eq!(binary.len, 64);
        assert_eq!(binary.data, value.to_ne_bytes().to_vec());

        let binary: TydiBinary = value.into();

        assert_eq!(binary.len, 64);
        assert_eq!(binary.data, value.to_ne_bytes().to_vec());
    }

    #[test]
    fn test_binary_from_f64() {
        let value: f64 = 3.14159;
        let binary = TydiBinary::from(value);

        let val2 = true;

        assert_eq!(binary.len, 64);
        assert_eq!(binary.data, value.to_ne_bytes().to_vec());
    }

    #[test]
    fn test_binary_from_string() {
        let value = 'm';
        let binary = TydiBinary::from(value);

        assert_eq!(binary.len, 8);
        // assert_eq!(binary.data, value.to_string().as_bytes().to_vec());
    }
}
