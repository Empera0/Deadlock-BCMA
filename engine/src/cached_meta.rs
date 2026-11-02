//! A match's metadata read from the copy the game left in Steam's HTTP cache, or downloaded from Valve's replay
//! servers, and turned into the same JSON the statistics service gives for the match, so that everything that reads
//! a match reads it unchanged.
//!
//! The file is what Valve serves at `replayNNN.valve.net/1422450/<match id>_<salt>.meta.bz2`: despite the name, zstd
//! compressed these days (bzip2 in older ones), holding a `CMsgMatchMetaData` whose `match_details` are a
//! `CMsgMatchMetaDataContents`. In Steam's cache it follows a header naming the request.

use std::io::Read;

use prost::Message;

#[allow(clippy::all, dead_code, non_camel_case_types)]
#[rustfmt::skip]
pub mod proto {
    include!("meta_proto.rs");
}

const ZSTD: &[u8] = &[0x28, 0xB5, 0x2F, 0xFD];
const BZIP2: &[u8] = b"BZh";
/// The cache's header is about a kilobyte; the body starts well within this.
const HEADER_MAX: usize = 16 * 1024;

/// Where the compressed file starts in a cache file (at 0 for the file itself).
pub fn body_start(file: &[u8]) -> Option<usize> {
    let head = &file[..file.len().min(HEADER_MAX)];
    let find = |magic: &[u8]| head.windows(magic.len()).position(|w| w == magic);
    match (find(ZSTD), find(BZIP2)) {
        (Some(z), Some(b)) => Some(z.min(b)),
        (z, b) => z.or(b),
    }
}

fn unpack(body: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = vec![];
    if body.starts_with(ZSTD) {
        ruzstd::decoding::StreamingDecoder::new(body)
            .map_err(|e| e.to_string())?
            .read_to_end(&mut out)
            .map_err(|e| format!("The match file could not be unpacked: {e}"))?;
    } else {
        bzip2_rs::DecoderReader::new(body).read_to_end(&mut out).map_err(|e| format!("The match file could not be unpacked: {e}"))?;
    }
    Ok(out)
}

/// The match's metadata, as the statistics service's `/v1/matches/{id}/metadata` JSON, from a cache file or from
/// the file Valve serves.
pub fn metadata_json(file: &[u8]) -> Result<String, String> {
    let start = body_start(file).ok_or("The match file is not in a known format.")?;
    let bytes = unpack(&file[start..])?;
    let meta = proto::CMsgMatchMetaData::decode(bytes.as_slice()).map_err(|e| format!("The match file could not be read: {e}"))?;
    let details = meta.match_details.unwrap_or_default();
    let contents = proto::CMsgMatchMetaDataContents::decode(details.as_slice()).map_err(|e| format!("The match file could not be read: {e}"))?;
    // Straight from the messages, as the service does: unset fields as null, enums as numbers, floats as floats.
    serde_json::to_string(&contents).map_err(|e| e.to_string())
}
