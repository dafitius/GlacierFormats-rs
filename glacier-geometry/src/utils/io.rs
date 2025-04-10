use std::{fmt, io};
use std::fmt::Write;
use std::io::{Read, Seek};
use std::string::FromUtf8Error;
use binrw::{BinRead, BinResult, BinWrite, Endian, NamedArgs};

// pub fn to_fixed_array(s: &str, size: usize) -> Vec<u8> {
//     let mut buffer = vec![0; size];
//     let bytes = s.as_bytes();
//     let length = bytes.len().min(size);
//     buffer[..length].copy_from_slice(&bytes[..length]);
//     buffer
// }

pub fn align_writer<W: io::Write + Seek>(writer: &mut W, num: usize) -> Result<(), io::Error> {
    let padding = (num - (writer.stream_position()? as usize % num)) % num;
    writer.write_all(vec![0; padding].as_slice())?;
    Ok(())
}

#[derive(NamedArgs, Clone)]
pub struct FixedStringArgs {
    pub count: usize,
}

#[derive(Clone, Eq, PartialEq, Default)]
pub struct FixedString(
    pub Vec<u8>,
);

impl BinRead for FixedString {
    type Args<'a> = FixedStringArgs;

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: Endian,
        args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let mut values = vec![];

        for _ in 0..args.count {
            let val = <u8>::read_options(reader, endian, ())?;
            values.push(val);
        }
        Ok(Self(values))
    }
}

impl BinWrite for FixedString {
    type Args<'a> = ();

    fn write_options<W: io::Write + Seek>(
        &self,
        writer: &mut W,
        endian: Endian,
        args: Self::Args<'_>,
    ) -> BinResult<()> {
        self.0.write_options(writer, endian, args)?;
        Ok(())
    }
}

impl From<&str> for FixedString {
    fn from(s: &str) -> Self {
        Self(s.as_bytes().to_vec())
    }
}

impl From<String> for FixedString {
    fn from(s: String) -> Self {
        Self(s.into_bytes())
    }
}

impl From<FixedString> for Vec<u8> {
    fn from(s: FixedString) -> Self {
        s.0
    }
}

impl TryFrom<FixedString> for String {
    type Error = FromUtf8Error;

    fn try_from(value: FixedString) -> Result<Self, Self::Error> {
        String::from_utf8(value.0).map(|str| str.trim_matches('\0').to_owned())
    }
}

impl core::ops::Deref for FixedString {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl core::ops::DerefMut for FixedString {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl fmt::Debug for FixedString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FixedString(\"")?;
        display_utf8(&self.0, f, str::escape_debug)?;
        write!(f, "\")")
    }
}

impl fmt::Display for FixedString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display_utf8(&self.0, f, str::chars)
    }
}

//Copied from <https://github.com/jam1garner/binrw/blob/master/binrw/src/strings.rs>
fn display_utf8<'a, Transformer: Fn(&'a str) -> O, O: Iterator<Item = char> + 'a>(
    mut input: &'a [u8],
    f: &mut fmt::Formatter<'_>,
    t: Transformer,
) -> fmt::Result {
    // Adapted from <https://doc.rust-lang.org/std/str/struct.Utf8Error.html>
    loop {
        match core::str::from_utf8(input) {
            Ok(valid) => {
                t(valid).try_for_each(|c| f.write_char(c))?;
                break;
            }
            Err(error) => {
                let (valid, after_valid) = input.split_at(error.valid_up_to());
                t(core::str::from_utf8(valid).unwrap()).try_for_each(|c| f.write_char(c))?;
                f.write_char(char::REPLACEMENT_CHARACTER)?;

                if let Some(invalid_sequence_length) = error.error_len() {
                    input = &after_valid[invalid_sequence_length..];
                } else {
                    break;
                }
            }
        }
    }
    Ok(())
}