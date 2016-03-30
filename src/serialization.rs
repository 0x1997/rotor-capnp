// See https://capnproto.org/encoding.html#serialization-over-a-stream for
// the specification.
use std::io::{Cursor, Write};

use byteorder::{ByteOrder, LittleEndian, WriteBytesExt};
use capnp::Result;
use capnp::message::{Builder, Reader, ReaderSegments};
use rotor_stream::Buf;

pub use capnp::{Error, Word};
pub use capnp::message::{Allocator as MessageAllocator, ReaderOptions};

/// Cap'n Proto message reader.
pub type MessageReader = Reader<OwnedSegments>;

/// Cap'n Proto message builder.
pub type MessageBuilder<A> = Builder<A>;

/// Cap'n Proto message serializer.
pub struct MessageWriter<'a>(pub &'a mut Buf);

pub fn read_segment_count(buf: &mut Buf) -> Result<usize> {
    let segment_count = <LittleEndian as ByteOrder>::read_u32(&buf[0..4]).wrapping_add(1) as usize;
    buf.consume(4);
    if segment_count >= 512 {
        Err(Error::failed(format!("Too many segments: {}", segment_count)))
    } else if segment_count == 0 {
        Err(Error::failed(format!("Too few segments: {}", segment_count)))
    } else {
        Ok(segment_count)
    }
}

pub fn read_segment_table(buf: &mut Buf,
                          segment_count: usize,
                          options: ReaderOptions)
                          -> Result<(usize, Vec<(usize, usize)>)> {
    let mut segment_slices = Vec::with_capacity(segment_count);
    let mut total_words: usize = 0;
    let mut i = 0;
    for _ in 0..segment_count {
        let segment_len = <LittleEndian as ByteOrder>::read_u32(&buf[i..i + 4]) as usize;
        segment_slices.push((total_words, total_words + segment_len));
        total_words += segment_len;
        i += 4;
    }
    buf.consume(segment_count * 4);
    if total_words as u64 > options.traversal_limit_in_words {
        Err(Error::failed(format!("Message has {} words, which is too \
            large. To increase the limit on the receiving end, see \
            capnp::message::ReaderOptions.",
                                  total_words)))
    } else {
        Ok((total_words, segment_slices))
    }
}

pub fn read_segments(buf: &mut Buf,
                     total_words: usize,
                     segment_slices: Vec<(usize, usize)>,
                     options: ReaderOptions)
                     -> Result<MessageReader> {
    let mut owned_space: Vec<Word> = Word::allocate_zeroed_vec(total_words);
    try!(buf.write_to(&mut Cursor::new(Word::words_to_bytes_mut(&mut owned_space[..]))));
    let segments = OwnedSegments {
        segment_slices: segment_slices,
        owned_space: owned_space,
    };
    Ok(Reader::new(segments, options))
}

pub struct OwnedSegments {
    segment_slices: Vec<(usize, usize)>,
    owned_space: Vec<Word>,
}

impl ReaderSegments for OwnedSegments {
    fn get_segment<'a>(&'a self, id: u32) -> Option<&'a [Word]> {
        if id < self.segment_slices.len() as u32 {
            let (a, b) = self.segment_slices[id as usize];
            Some(&self.owned_space[a..b])
        } else {
            None
        }
    }
}

impl<'a> MessageWriter<'a> {
    /// Serialize and write the message to the connection buffer.
    pub fn write<A: MessageAllocator>(&mut self, message: &MessageBuilder<A>) {
        let segments = message.get_segments_for_output();
        self.0.write_u32::<LittleEndian>(segments.len() as u32 - 1).unwrap();
        let segments: &[&[Word]] = &*segments;
        for segment in segments {
            self.0.write_u32::<LittleEndian>(segment.len() as u32).unwrap();
        }
        for &segment in segments {
            self.0.write(Word::words_to_bytes(segment)).unwrap();
        }
    }
}
