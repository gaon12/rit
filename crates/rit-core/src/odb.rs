use crate::object::sha1_bytes;
use crate::{
    GitObject, ObjectId, ObjectKind, Result, RitError, hash_object, object::parse_loose_object,
};
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Reader for Git loose objects under `.git/objects`.
#[derive(Clone, Debug)]
pub struct LooseObjectDb {
    objects_dir: PathBuf,
}

/// Stored packfile metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredPack {
    /// Pack checksum stored in the pack trailer.
    pub checksum: ObjectId,
    /// Number of objects declared by the pack header.
    pub object_count: u32,
    /// Path of the stored `.pack` file.
    pub path: PathBuf,
}

/// Stored pack index metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredPackIndex {
    /// Pack checksum copied into the index trailer.
    pub pack_checksum: ObjectId,
    /// Number of indexed objects.
    pub object_count: u32,
    /// Path of the stored `.idx` file.
    pub path: PathBuf,
}

impl LooseObjectDb {
    /// Creates a loose object reader rooted at an `objects` directory.
    pub fn new(objects_dir: impl Into<PathBuf>) -> Self {
        Self {
            objects_dir: objects_dir.into(),
        }
    }

    /// Reads and validates one loose object.
    pub fn read_object(&self, object_id: ObjectId) -> Result<GitObject> {
        let path = self.loose_object_path(object_id);
        if path.is_file() {
            let compressed = fs::read(&path).map_err(|source| RitError::io(&path, source))?;
            let mut decoder = ZlibDecoder::new(compressed.as_slice());
            let mut raw = Vec::new();
            decoder
                .read_to_end(&mut raw)
                .map_err(|source| RitError::io(&path, source))?;
            return parse_loose_object(&raw);
        }

        self.read_packed_object(object_id)?
            .ok_or_else(|| RitError::ObjectNotFound {
                object_id: object_id.to_hex(),
            })
    }

    /// Writes a loose object if it is not already present.
    pub fn write_object(&self, kind: ObjectKind, data: &[u8]) -> Result<ObjectId> {
        let object_id = hash_object(kind, data);
        let path = self.loose_object_path(object_id);
        if path.exists() {
            return Ok(object_id);
        }

        let parent = path
            .parent()
            .ok_or_else(|| RitError::invalid_input("loose object path has no parent"))?;
        fs::create_dir_all(parent).map_err(|source| RitError::io(parent, source))?;

        let mut raw = Vec::new();
        raw.extend_from_slice(kind.to_str().as_bytes());
        raw.push(b' ');
        raw.extend_from_slice(data.len().to_string().as_bytes());
        raw.push(0);
        raw.extend_from_slice(data);

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(&raw)
            .map_err(|source| RitError::io(&path, source))?;
        let compressed = encoder
            .finish()
            .map_err(|source| RitError::io(&path, source))?;
        write_new_file_atomically(&path, &compressed)
            .or_else(|error| if path.exists() { Ok(()) } else { Err(error) })?;
        Ok(object_id)
    }

    /// Validates and stores a received packfile.
    ///
    /// This writes only the `.pack` file. A later transport step must build the
    /// matching `.idx` file before normal object lookup can read from it.
    pub fn store_pack(&self, pack_bytes: &[u8]) -> Result<StoredPack> {
        let (checksum, object_count) = validate_pack_bytes(pack_bytes)?;
        let pack_dir = self.objects_dir.join("pack");
        fs::create_dir_all(&pack_dir).map_err(|source| RitError::io(&pack_dir, source))?;
        let path = pack_dir.join(format!("pack-{}.pack", checksum.to_hex()));
        if !path.exists() {
            write_new_file_atomically(&path, pack_bytes)?;
        }
        Ok(StoredPack {
            checksum,
            object_count,
            path,
        })
    }

    /// Builds and stores a Git pack index v2 for a received packfile.
    pub fn write_pack_index(&self, pack_bytes: &[u8]) -> Result<StoredPackIndex> {
        let (pack_checksum, object_count, mut entries) =
            self.collect_pack_index_entries(pack_bytes)?;
        entries.sort_by(|left, right| left.object_id.as_bytes().cmp(right.object_id.as_bytes()));

        let mut index = Vec::new();
        index.extend_from_slice(b"\xfftOc");
        index.extend_from_slice(&2_u32.to_be_bytes());
        write_pack_index_fanout(&mut index, &entries);
        for entry in &entries {
            index.extend_from_slice(entry.object_id.as_bytes());
        }
        for entry in &entries {
            index.extend_from_slice(&entry.crc32.to_be_bytes());
        }
        let mut large_offsets = Vec::new();
        for entry in &entries {
            if entry.offset <= 0x7fff_ffff {
                index.extend_from_slice(&(entry.offset as u32).to_be_bytes());
            } else {
                let large_index = large_offsets.len() as u32;
                index.extend_from_slice(&(0x8000_0000 | large_index).to_be_bytes());
                large_offsets.push(entry.offset);
            }
        }
        for offset in large_offsets {
            index.extend_from_slice(&offset.to_be_bytes());
        }
        index.extend_from_slice(pack_checksum.as_bytes());
        let index_checksum = sha1_bytes(&index);
        index.extend_from_slice(&index_checksum);

        let pack_dir = self.objects_dir.join("pack");
        fs::create_dir_all(&pack_dir).map_err(|source| RitError::io(&pack_dir, source))?;
        let path = pack_dir.join(format!("pack-{}.idx", pack_checksum.to_hex()));
        if !path.exists() {
            write_new_file_atomically(&path, &index)?;
        }
        Ok(StoredPackIndex {
            pack_checksum,
            object_count,
            path,
        })
    }

    /// Unpacks supported pack entries into loose objects.
    ///
    /// Whole objects and deltas whose base has already appeared in the same
    /// pack, or already exists in the object database, are supported. Pack
    /// indexes are still handled separately.
    pub fn unpack_pack_as_loose(&self, pack_bytes: &[u8]) -> Result<Vec<ObjectId>> {
        let (_, object_count) = validate_pack_bytes(pack_bytes)?;
        let trailer_start = pack_bytes.len() - 20;
        let mut position = 12;
        let mut objects_by_offset: HashMap<u64, GitObject> = HashMap::new();
        let mut objects_by_id: HashMap<ObjectId, GitObject> = HashMap::new();
        let mut object_ids = Vec::new();

        for _ in 0..object_count {
            let object_offset = position;
            let (kind, size) = read_pack_object_header_from_bytes(pack_bytes, &mut position)?;
            let object = match kind {
                PackObjectKind::Whole(kind) => {
                    let (data, consumed) = read_compressed_pack_payload_from_bytes(
                        pack_bytes,
                        position,
                        trailer_start,
                        size,
                    )?;
                    position += consumed;
                    GitObject { kind, data }
                }
                PackObjectKind::OffsetDelta => {
                    let base_offset = read_offset_delta_base_from_bytes(
                        pack_bytes,
                        &mut position,
                        object_offset,
                    )?;
                    let (delta, consumed) = read_compressed_pack_payload_from_bytes(
                        pack_bytes,
                        position,
                        trailer_start,
                        size,
                    )?;
                    position += consumed;
                    let base = objects_by_offset
                        .get(&base_offset)
                        .cloned()
                        .ok_or_else(|| RitError::invalid_input("offset delta base is missing"))?;
                    apply_delta_object(base, &delta)?
                }
                PackObjectKind::RefDelta => {
                    let base_id = read_ref_delta_base_from_bytes(pack_bytes, &mut position)?;
                    let (delta, consumed) = read_compressed_pack_payload_from_bytes(
                        pack_bytes,
                        position,
                        trailer_start,
                        size,
                    )?;
                    position += consumed;
                    let base = match objects_by_id.get(&base_id) {
                        Some(base) => base.clone(),
                        None => self.read_object(base_id)?,
                    };
                    apply_delta_object(base, &delta)?
                }
            };
            let object_id = self.write_object(object.kind, &object.data)?;
            objects_by_offset.insert(object_offset as u64, object.clone());
            objects_by_id.insert(object_id, object);
            object_ids.push(object_id);
        }

        if position != trailer_start {
            return Err(RitError::invalid_input(
                "packfile has trailing data before checksum",
            ));
        }
        Ok(object_ids)
    }

    /// Returns the path where a loose object should live.
    pub fn loose_object_path(&self, object_id: ObjectId) -> PathBuf {
        let hex = object_id.to_hex();
        self.objects_dir.join(&hex[..2]).join(&hex[2..])
    }

    /// Returns the root objects directory.
    pub fn objects_dir(&self) -> &Path {
        &self.objects_dir
    }

    /// Finds one object ID by a hexadecimal prefix. Ambiguous prefixes return
    /// an explicit error instead of guessing.
    pub fn find_object_id_by_prefix(&self, prefix: &str) -> Result<Option<ObjectId>> {
        if prefix.len() < 4 || prefix.len() > 40 {
            return Ok(None);
        }
        if !prefix
            .chars()
            .all(|character| character.is_ascii_hexdigit())
        {
            return Ok(None);
        }
        let prefix = prefix.to_ascii_lowercase();
        let mut found = None;

        for object_id in self.loose_object_ids_with_prefix(&prefix)? {
            remember_unique_object_id(&mut found, object_id, &prefix)?;
        }
        for object_id in self.packed_object_ids_with_prefix(&prefix)? {
            remember_unique_object_id(&mut found, object_id, &prefix)?;
        }

        Ok(found)
    }

    fn read_packed_object(&self, object_id: ObjectId) -> Result<Option<GitObject>> {
        let pack_dir = self.objects_dir.join("pack");
        if !pack_dir.exists() {
            return Ok(None);
        }

        for entry in fs::read_dir(&pack_dir).map_err(|source| RitError::io(&pack_dir, source))? {
            let entry = entry.map_err(|source| RitError::io(&pack_dir, source))?;
            let index_path = entry.path();
            if index_path
                .extension()
                .and_then(|extension| extension.to_str())
                != Some("idx")
            {
                continue;
            }
            let Some(offset) = find_pack_offset(&index_path, object_id)? else {
                continue;
            };
            let pack_path = index_path.with_extension("pack");
            return read_pack_object_at(&pack_path, offset, &self.objects_dir, 0).map(Some);
        }

        Ok(None)
    }

    fn loose_object_ids_with_prefix(&self, prefix: &str) -> Result<Vec<ObjectId>> {
        let mut matches = Vec::new();
        let (directory_prefix, file_prefix) = prefix.split_at(2);
        let directory = self.objects_dir.join(directory_prefix);
        if !directory.exists() {
            return Ok(matches);
        }
        for entry in fs::read_dir(&directory).map_err(|source| RitError::io(&directory, source))? {
            let entry = entry.map_err(|source| RitError::io(&directory, source))?;
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if file_name.len() != 38 || !file_name.starts_with(file_prefix) {
                continue;
            }
            matches.push(ObjectId::from_hex(&format!(
                "{directory_prefix}{file_name}"
            ))?);
        }
        Ok(matches)
    }

    fn packed_object_ids_with_prefix(&self, prefix: &str) -> Result<Vec<ObjectId>> {
        let pack_dir = self.objects_dir.join("pack");
        let mut matches = Vec::new();
        if !pack_dir.exists() {
            return Ok(matches);
        }
        for entry in fs::read_dir(&pack_dir).map_err(|source| RitError::io(&pack_dir, source))? {
            let entry = entry.map_err(|source| RitError::io(&pack_dir, source))?;
            let index_path = entry.path();
            if index_path
                .extension()
                .and_then(|extension| extension.to_str())
                != Some("idx")
            {
                continue;
            }
            matches.extend(pack_object_ids_with_prefix(&index_path, prefix)?);
        }
        Ok(matches)
    }

    fn collect_pack_index_entries(
        &self,
        pack_bytes: &[u8],
    ) -> Result<(ObjectId, u32, Vec<PackIndexBuildEntry>)> {
        let (pack_checksum, object_count) = validate_pack_bytes(pack_bytes)?;
        let trailer_start = pack_bytes.len() - 20;
        let mut position = 12;
        let mut objects_by_offset: HashMap<u64, GitObject> = HashMap::new();
        let mut objects_by_id: HashMap<ObjectId, GitObject> = HashMap::new();
        let mut entries = Vec::new();

        for _ in 0..object_count {
            let object_offset = position;
            let (kind, size) = read_pack_object_header_from_bytes(pack_bytes, &mut position)?;
            let object = match kind {
                PackObjectKind::Whole(kind) => {
                    let (data, consumed) = read_compressed_pack_payload_from_bytes(
                        pack_bytes,
                        position,
                        trailer_start,
                        size,
                    )?;
                    position += consumed;
                    GitObject { kind, data }
                }
                PackObjectKind::OffsetDelta => {
                    let base_offset = read_offset_delta_base_from_bytes(
                        pack_bytes,
                        &mut position,
                        object_offset,
                    )?;
                    let (delta, consumed) = read_compressed_pack_payload_from_bytes(
                        pack_bytes,
                        position,
                        trailer_start,
                        size,
                    )?;
                    position += consumed;
                    let base = objects_by_offset
                        .get(&base_offset)
                        .cloned()
                        .ok_or_else(|| RitError::invalid_input("offset delta base is missing"))?;
                    apply_delta_object(base, &delta)?
                }
                PackObjectKind::RefDelta => {
                    let base_id = read_ref_delta_base_from_bytes(pack_bytes, &mut position)?;
                    let (delta, consumed) = read_compressed_pack_payload_from_bytes(
                        pack_bytes,
                        position,
                        trailer_start,
                        size,
                    )?;
                    position += consumed;
                    let base = match objects_by_id.get(&base_id) {
                        Some(base) => base.clone(),
                        None => self.read_object(base_id)?,
                    };
                    apply_delta_object(base, &delta)?
                }
            };
            let object_id = hash_object(object.kind, &object.data);
            let offset = object_offset as u64;
            entries.push(PackIndexBuildEntry {
                object_id,
                offset,
                crc32: crc32(&pack_bytes[object_offset..position]),
            });
            objects_by_offset.insert(offset, object.clone());
            objects_by_id.insert(object_id, object);
        }

        if position != trailer_start {
            return Err(RitError::invalid_input(
                "packfile has trailing data before checksum",
            ));
        }
        Ok((pack_checksum, object_count, entries))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PackIndexBuildEntry {
    object_id: ObjectId,
    offset: u64,
    crc32: u32,
}

fn validate_pack_bytes(pack_bytes: &[u8]) -> Result<(ObjectId, u32)> {
    if pack_bytes.len() < 32 {
        return Err(RitError::invalid_input("packfile is too short"));
    }
    if &pack_bytes[..4] != b"PACK" {
        return Err(RitError::invalid_input(
            "packfile is missing PACK signature",
        ));
    }
    let version = read_u32(pack_bytes, 4)?;
    if version != 2 && version != 3 {
        return Err(RitError::invalid_input(format!(
            "unsupported pack version: {version}"
        )));
    }
    let object_count = read_u32(pack_bytes, 8)?;
    let trailer_start = pack_bytes.len() - 20;
    let expected_checksum = sha1_bytes(&pack_bytes[..trailer_start]);
    if pack_bytes[trailer_start..] != expected_checksum {
        return Err(RitError::invalid_input("packfile checksum mismatch"));
    }
    Ok((ObjectId::from_bytes(expected_checksum), object_count))
}

fn read_pack_object_header_from_bytes(
    bytes: &[u8],
    position: &mut usize,
) -> Result<(PackObjectKind, usize)> {
    let first = read_pack_byte_from_bytes(bytes, position)?;
    let object_type = (first >> 4) & 0x07;
    let mut size = (first & 0x0f) as usize;
    let mut shift = 4;
    let mut byte = first;
    while byte & 0x80 != 0 {
        byte = read_pack_byte_from_bytes(bytes, position)?;
        size |= ((byte & 0x7f) as usize) << shift;
        shift += 7;
    }
    let kind = match object_type {
        1 => PackObjectKind::Whole(ObjectKind::Commit),
        2 => PackObjectKind::Whole(ObjectKind::Tree),
        3 => PackObjectKind::Whole(ObjectKind::Blob),
        4 => PackObjectKind::Whole(ObjectKind::Tag),
        6 => PackObjectKind::OffsetDelta,
        7 => PackObjectKind::RefDelta,
        _ => {
            return Err(RitError::invalid_input(format!(
                "unknown pack object type: {object_type}"
            )));
        }
    };
    Ok((kind, size))
}

fn read_pack_byte_from_bytes(bytes: &[u8], position: &mut usize) -> Result<u8> {
    let byte = *bytes
        .get(*position)
        .ok_or_else(|| RitError::invalid_input("pack object header is truncated"))?;
    *position += 1;
    Ok(byte)
}

fn read_offset_delta_base_from_bytes(
    bytes: &[u8],
    position: &mut usize,
    object_offset: usize,
) -> Result<u64> {
    let mut byte = read_pack_byte_from_bytes(bytes, position)?;
    let mut distance = (byte & 0x7f) as u64;
    while byte & 0x80 != 0 {
        byte = read_pack_byte_from_bytes(bytes, position)?;
        distance = ((distance + 1) << 7) | (byte & 0x7f) as u64;
    }
    (object_offset as u64)
        .checked_sub(distance)
        .ok_or_else(|| RitError::invalid_input("pack offset delta points before pack start"))
}

fn read_ref_delta_base_from_bytes(bytes: &[u8], position: &mut usize) -> Result<ObjectId> {
    let end = position
        .checked_add(20)
        .ok_or_else(|| RitError::invalid_input("ref delta base offset overflows"))?;
    let base = bytes
        .get(*position..end)
        .ok_or_else(|| RitError::invalid_input("ref delta base object id is truncated"))?;
    let mut base_id = [0_u8; 20];
    base_id.copy_from_slice(base);
    *position = end;
    Ok(ObjectId::from_bytes(base_id))
}

fn read_compressed_pack_payload_from_bytes(
    bytes: &[u8],
    position: usize,
    trailer_start: usize,
    expected_size: usize,
) -> Result<(Vec<u8>, usize)> {
    let payload = bytes
        .get(position..trailer_start)
        .ok_or_else(|| RitError::invalid_input("pack payload starts after trailer"))?;
    let mut decoder = ZlibDecoder::new(payload);
    let mut data = Vec::new();
    decoder
        .read_to_end(&mut data)
        .map_err(|source| RitError::io("pack-bytes", source))?;
    if data.len() != expected_size {
        return Err(RitError::invalid_input(format!(
            "pack object size mismatch: header says {expected_size}, payload is {}",
            data.len()
        )));
    }
    Ok((data, decoder.total_in() as usize))
}

fn remember_unique_object_id(
    found: &mut Option<ObjectId>,
    object_id: ObjectId,
    prefix: &str,
) -> Result<()> {
    if found.is_some_and(|existing| existing != object_id) {
        return Err(RitError::invalid_input(format!(
            "short object id is ambiguous: {prefix}"
        )));
    }
    *found = Some(object_id);
    Ok(())
}

fn find_pack_offset(index_path: &Path, object_id: ObjectId) -> Result<Option<u64>> {
    let bytes = fs::read(index_path).map_err(|source| RitError::io(index_path, source))?;
    if bytes.len() < 8 || &bytes[..4] != b"\xfftOc" {
        return Err(RitError::invalid_input(format!(
            "unsupported pack index format: {}",
            index_path.display()
        )));
    }
    let version = read_u32(&bytes, 4)?;
    if version != 2 {
        return Err(RitError::invalid_input(format!(
            "unsupported pack index version: {version}"
        )));
    }

    let fanout_start = 8;
    let object_count = read_u32(&bytes, fanout_start + 255 * 4)? as usize;
    let names_start = fanout_start + 256 * 4;
    let crc_start = names_start + object_count * 20;
    let offsets_start = crc_start + object_count * 4;
    let large_offsets_start = offsets_start + object_count * 4;
    if bytes.len() < large_offsets_start {
        return Err(RitError::invalid_input("pack index is truncated"));
    }

    let first_byte = object_id.as_bytes()[0] as usize;
    let low = if first_byte == 0 {
        0
    } else {
        read_u32(&bytes, fanout_start + (first_byte - 1) * 4)? as usize
    };
    let high = read_u32(&bytes, fanout_start + first_byte * 4)? as usize;
    let mut left = low;
    let mut right = high;

    while left < right {
        let middle = (left + right) / 2;
        let name_start = names_start + middle * 20;
        let name = bytes
            .get(name_start..name_start + 20)
            .ok_or_else(|| RitError::invalid_input("pack index object name table is truncated"))?;
        match name.cmp(object_id.as_bytes()) {
            std::cmp::Ordering::Less => left = middle + 1,
            std::cmp::Ordering::Greater => right = middle,
            std::cmp::Ordering::Equal => {
                let offset_value = read_u32(&bytes, offsets_start + middle * 4)?;
                if offset_value & 0x8000_0000 == 0 {
                    return Ok(Some(offset_value as u64));
                }
                let large_index = (offset_value & 0x7fff_ffff) as usize;
                return Ok(Some(read_u64(
                    &bytes,
                    large_offsets_start + large_index * 8,
                )?));
            }
        }
    }

    Ok(None)
}

fn pack_object_ids_with_prefix(index_path: &Path, prefix: &str) -> Result<Vec<ObjectId>> {
    let bytes = fs::read(index_path).map_err(|source| RitError::io(index_path, source))?;
    let (object_count, names_start) = parse_pack_index_header(&bytes, index_path)?;
    let mut matches = Vec::new();
    for index in 0..object_count {
        let name_start = names_start + index * 20;
        let name = bytes
            .get(name_start..name_start + 20)
            .ok_or_else(|| RitError::invalid_input("pack index object name table is truncated"))?;
        let mut object_bytes = [0_u8; 20];
        object_bytes.copy_from_slice(name);
        let object_id = ObjectId::from_bytes(object_bytes);
        if object_id.to_hex().starts_with(prefix) {
            matches.push(object_id);
        }
    }
    Ok(matches)
}

fn parse_pack_index_header(bytes: &[u8], index_path: &Path) -> Result<(usize, usize)> {
    if bytes.len() < 8 || &bytes[..4] != b"\xfftOc" {
        return Err(RitError::invalid_input(format!(
            "unsupported pack index format: {}",
            index_path.display()
        )));
    }
    let version = read_u32(bytes, 4)?;
    if version != 2 {
        return Err(RitError::invalid_input(format!(
            "unsupported pack index version: {version}"
        )));
    }
    let fanout_start = 8;
    let object_count = read_u32(bytes, fanout_start + 255 * 4)? as usize;
    Ok((object_count, fanout_start + 256 * 4))
}

fn write_pack_index_fanout(output: &mut Vec<u8>, entries: &[PackIndexBuildEntry]) {
    let mut counts = [0_u32; 256];
    for entry in entries {
        counts[entry.object_id.as_bytes()[0] as usize] += 1;
    }
    let mut total = 0_u32;
    for count in counts {
        total += count;
        output.extend_from_slice(&total.to_be_bytes());
    }
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for byte in bytes {
        crc ^= *byte as u32;
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn read_pack_object_at(
    pack_path: &Path,
    offset: u64,
    objects_dir: &Path,
    depth: usize,
) -> Result<GitObject> {
    if depth > 64 {
        return Err(RitError::invalid_input("pack delta chain is too deep"));
    }
    let mut file = fs::File::open(pack_path).map_err(|source| RitError::io(pack_path, source))?;
    validate_pack_header(&mut file, pack_path)?;
    file.seek(SeekFrom::Start(offset))
        .map_err(|source| RitError::io(pack_path, source))?;

    let (kind, size) = read_pack_object_header(&mut file, pack_path)?;
    match kind {
        PackObjectKind::Whole(kind) => read_whole_pack_object(file, pack_path, kind, size),
        PackObjectKind::OffsetDelta => {
            let base_offset = read_offset_delta_base(&mut file, pack_path, offset)?;
            let delta = read_compressed_pack_payload(file, pack_path, size)?;
            let base = read_pack_object_at(pack_path, base_offset, objects_dir, depth + 1)?;
            apply_delta_object(base, &delta)
        }
        PackObjectKind::RefDelta => {
            let mut base_id = [0_u8; 20];
            file.read_exact(&mut base_id)
                .map_err(|source| RitError::io(pack_path, source))?;
            let delta = read_compressed_pack_payload(file, pack_path, size)?;
            let base = read_ref_delta_base(
                pack_path,
                objects_dir,
                ObjectId::from_bytes(base_id),
                depth + 1,
            )?;
            apply_delta_object(base, &delta)
        }
    }
}

fn read_whole_pack_object(
    file: fs::File,
    pack_path: &Path,
    kind: ObjectKind,
    size: usize,
) -> Result<GitObject> {
    let data = read_compressed_pack_payload(file, pack_path, size)?;
    Ok(GitObject { kind, data })
}

fn read_compressed_pack_payload(
    file: fs::File,
    pack_path: &Path,
    expected_size: usize,
) -> Result<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(file);
    let mut data = Vec::new();
    decoder
        .read_to_end(&mut data)
        .map_err(|source| RitError::io(pack_path, source))?;
    if data.len() != expected_size {
        return Err(RitError::invalid_input(format!(
            "pack object size mismatch: header says {expected_size}, payload is {}",
            data.len()
        )));
    }
    Ok(data)
}

fn read_ref_delta_base(
    pack_path: &Path,
    objects_dir: &Path,
    base_id: ObjectId,
    depth: usize,
) -> Result<GitObject> {
    let index_path = pack_path.with_extension("idx");
    if let Some(base_offset) = find_pack_offset(&index_path, base_id)? {
        return read_pack_object_at(pack_path, base_offset, objects_dir, depth);
    }
    LooseObjectDb::new(objects_dir).read_object(base_id)
}

fn read_offset_delta_base(
    file: &mut fs::File,
    pack_path: &Path,
    object_offset: u64,
) -> Result<u64> {
    let mut byte = read_byte(file, pack_path)?;
    let mut distance = (byte & 0x7f) as u64;
    while byte & 0x80 != 0 {
        byte = read_byte(file, pack_path)?;
        distance = ((distance + 1) << 7) | (byte & 0x7f) as u64;
    }
    object_offset
        .checked_sub(distance)
        .ok_or_else(|| RitError::invalid_input("pack offset delta points before pack start"))
}

fn apply_delta_object(base: GitObject, delta: &[u8]) -> Result<GitObject> {
    let mut position = 0;
    let source_size = read_delta_size(delta, &mut position)?;
    let target_size = read_delta_size(delta, &mut position)?;
    if source_size != base.data.len() {
        return Err(RitError::invalid_input(format!(
            "delta source size mismatch: base is {}, delta expects {source_size}",
            base.data.len()
        )));
    }

    let mut output = Vec::with_capacity(target_size);
    while position < delta.len() {
        let instruction = delta[position];
        position += 1;
        if instruction & 0x80 != 0 {
            let (copy_offset, copy_size) = read_delta_copy(delta, &mut position, instruction)?;
            let copy_end = copy_offset
                .checked_add(copy_size)
                .ok_or_else(|| RitError::invalid_input("delta copy range overflows"))?;
            let slice = base
                .data
                .get(copy_offset..copy_end)
                .ok_or_else(|| RitError::invalid_input("delta copy range is outside base"))?;
            output.extend_from_slice(slice);
        } else if instruction != 0 {
            let insert_size = instruction as usize;
            let insert_end = position
                .checked_add(insert_size)
                .ok_or_else(|| RitError::invalid_input("delta insert range overflows"))?;
            let slice = delta
                .get(position..insert_end)
                .ok_or_else(|| RitError::invalid_input("delta insert range is truncated"))?;
            output.extend_from_slice(slice);
            position = insert_end;
        } else {
            return Err(RitError::invalid_input("delta instruction 0 is invalid"));
        }
    }

    if output.len() != target_size {
        return Err(RitError::invalid_input(format!(
            "delta target size mismatch: output is {}, delta expects {target_size}",
            output.len()
        )));
    }
    Ok(GitObject {
        kind: base.kind,
        data: output,
    })
}

fn read_delta_size(delta: &[u8], position: &mut usize) -> Result<usize> {
    let mut size = 0_usize;
    let mut shift = 0;
    loop {
        let byte = *delta
            .get(*position)
            .ok_or_else(|| RitError::invalid_input("delta size is truncated"))?;
        *position += 1;
        size |= ((byte & 0x7f) as usize) << shift;
        if byte & 0x80 == 0 {
            return Ok(size);
        }
        shift += 7;
    }
}

fn read_delta_copy(delta: &[u8], position: &mut usize, instruction: u8) -> Result<(usize, usize)> {
    let mut offset = 0_usize;
    let mut size = 0_usize;
    if instruction & 0x01 != 0 {
        offset |= read_delta_instruction_byte(delta, position)? as usize;
    }
    if instruction & 0x02 != 0 {
        offset |= (read_delta_instruction_byte(delta, position)? as usize) << 8;
    }
    if instruction & 0x04 != 0 {
        offset |= (read_delta_instruction_byte(delta, position)? as usize) << 16;
    }
    if instruction & 0x08 != 0 {
        offset |= (read_delta_instruction_byte(delta, position)? as usize) << 24;
    }
    if instruction & 0x10 != 0 {
        size |= read_delta_instruction_byte(delta, position)? as usize;
    }
    if instruction & 0x20 != 0 {
        size |= (read_delta_instruction_byte(delta, position)? as usize) << 8;
    }
    if instruction & 0x40 != 0 {
        size |= (read_delta_instruction_byte(delta, position)? as usize) << 16;
    }
    if size == 0 {
        size = 0x10000;
    }
    Ok((offset, size))
}

fn read_delta_instruction_byte(delta: &[u8], position: &mut usize) -> Result<u8> {
    let byte = *delta
        .get(*position)
        .ok_or_else(|| RitError::invalid_input("delta instruction is truncated"))?;
    *position += 1;
    Ok(byte)
}

fn validate_pack_header(file: &mut fs::File, pack_path: &Path) -> Result<()> {
    let mut header = [0_u8; 12];
    file.read_exact(&mut header)
        .map_err(|source| RitError::io(pack_path, source))?;
    if &header[..4] != b"PACK" {
        return Err(RitError::invalid_input(format!(
            "invalid pack header: {}",
            pack_path.display()
        )));
    }
    let version = u32::from_be_bytes([header[4], header[5], header[6], header[7]]);
    if !matches!(version, 2 | 3) {
        return Err(RitError::invalid_input(format!(
            "unsupported pack version: {version}"
        )));
    }
    Ok(())
}

enum PackObjectKind {
    Whole(ObjectKind),
    OffsetDelta,
    RefDelta,
}

fn read_pack_object_header(
    file: &mut fs::File,
    pack_path: &Path,
) -> Result<(PackObjectKind, usize)> {
    let mut byte = read_byte(file, pack_path)?;
    let object_type = (byte >> 4) & 0b111;
    let mut size = (byte & 0b1111) as usize;
    let mut shift = 4;
    while byte & 0b1000_0000 != 0 {
        byte = read_byte(file, pack_path)?;
        size |= ((byte & 0b0111_1111) as usize) << shift;
        shift += 7;
    }
    let kind = match object_type {
        1 => PackObjectKind::Whole(ObjectKind::Commit),
        2 => PackObjectKind::Whole(ObjectKind::Tree),
        3 => PackObjectKind::Whole(ObjectKind::Blob),
        4 => PackObjectKind::Whole(ObjectKind::Tag),
        6 => PackObjectKind::OffsetDelta,
        7 => PackObjectKind::RefDelta,
        _ => {
            return Err(RitError::invalid_input(format!(
                "unknown pack object type: {object_type}"
            )));
        }
    };
    Ok((kind, size))
}

fn read_byte(file: &mut fs::File, path: &Path) -> Result<u8> {
    let mut byte = [0_u8; 1];
    file.read_exact(&mut byte)
        .map_err(|source| RitError::io(path, source))?;
    Ok(byte[0])
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| RitError::invalid_input("pack index integer is truncated"))?;
    Ok(u32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    let slice = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| RitError::invalid_input("pack index large offset is truncated"))?;
    Ok(u64::from_be_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ]))
}

fn write_new_file_atomically(path: &Path, contents: &[u8]) -> Result<()> {
    let temp_path = path.with_extension(format!("tmp-{}", std::process::id()));
    {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|source| RitError::io(&temp_path, source))?;
        file.write_all(contents)
            .map_err(|source| RitError::io(&temp_path, source))?;
        file.sync_all()
            .map_err(|source| RitError::io(&temp_path, source))?;
    }
    fs::rename(&temp_path, path).map_err(|source| RitError::io(path, source))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{LooseObjectDb, apply_delta_object};
    use crate::{GitObject, ObjectKind, hash_object, object::sha1_bytes};
    use flate2::{Compression, write::ZlibEncoder};
    use std::{
        fs,
        io::Write,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn applies_git_pack_delta_copy_and_insert_instructions() {
        let base = GitObject {
            kind: ObjectKind::Blob,
            data: b"abcdef".to_vec(),
        };
        let delta = [6, 7, 0x90, 2, 2, b'X', b'Y', 0x91, 4, 2, 1, b'!'];

        let object = apply_delta_object(base, &delta).expect("delta should apply");

        assert_eq!(object.kind, ObjectKind::Blob);
        assert_eq!(object.data, b"abXYef!");
    }

    #[test]
    fn stores_received_pack_by_trailer_checksum() {
        let objects_dir = temp_path("store-pack").join("objects");
        let database = LooseObjectDb::new(&objects_dir);
        let pack = empty_pack();
        let stored = database.store_pack(&pack).expect("pack should store");

        assert_eq!(stored.object_count, 0);
        assert!(stored.path.is_file());
        assert_eq!(fs::read(&stored.path).expect("pack bytes"), pack);
        let expected_name = format!("pack-{}.pack", stored.checksum);
        assert_eq!(
            stored.path.file_name().and_then(|name| name.to_str()),
            Some(expected_name.as_str())
        );
    }

    #[test]
    fn rejects_received_pack_with_bad_checksum() {
        let objects_dir = temp_path("bad-pack").join("objects");
        let database = LooseObjectDb::new(&objects_dir);
        let mut pack = empty_pack();
        let last = pack.last_mut().expect("trailer byte exists");
        *last ^= 1;

        let error = database
            .store_pack(&pack)
            .expect_err("checksum should fail");

        assert!(error.to_string().contains("checksum"));
        assert!(!objects_dir.join("pack").exists());
    }

    #[test]
    fn writes_pack_index_for_received_pack() {
        let objects_dir = temp_path("write-pack-index").join("objects");
        let database = LooseObjectDb::new(&objects_dir);
        let pack = pack_with_one_blob(b"hello");
        let stored = database.store_pack(&pack).expect("pack should store");
        let index = database
            .write_pack_index(&pack)
            .expect("index should write");

        assert_eq!(index.pack_checksum, stored.checksum);
        assert_eq!(index.object_count, 1);
        assert!(index.path.is_file());
        let expected_name = format!("pack-{}.idx", stored.checksum);
        assert_eq!(
            index.path.file_name().and_then(|name| name.to_str()),
            Some(expected_name.as_str())
        );
        let object_id = hash_object(ObjectKind::Blob, b"hello");
        let object = database.read_object(object_id).expect("packed object");
        assert_eq!(object.kind, ObjectKind::Blob);
        assert_eq!(object.data, b"hello");
    }

    #[test]
    fn writes_pack_index_for_delta_pack() {
        let objects_dir = temp_path("write-delta-pack-index").join("objects");
        let database = LooseObjectDb::new(&objects_dir);
        let pack = pack_with_offset_delta_blob();
        database.store_pack(&pack).expect("pack should store");
        database
            .write_pack_index(&pack)
            .expect("index should write");

        let object_id = hash_object(ObjectKind::Blob, b"hello!");
        let object = database
            .read_object(object_id)
            .expect("packed delta object");
        assert_eq!(object.kind, ObjectKind::Blob);
        assert_eq!(object.data, b"hello!");
    }

    #[test]
    fn unpacks_whole_pack_objects_as_loose_objects() {
        let objects_dir = temp_path("unpack-whole").join("objects");
        let database = LooseObjectDb::new(&objects_dir);
        let pack = pack_with_one_blob(b"hello");

        let object_ids = database
            .unpack_pack_as_loose(&pack)
            .expect("pack should unpack");

        let expected_id = hash_object(ObjectKind::Blob, b"hello");
        assert_eq!(object_ids, [expected_id]);
        let object = database.read_object(expected_id).expect("loose object");
        assert_eq!(object.kind, ObjectKind::Blob);
        assert_eq!(object.data, b"hello");
    }

    #[test]
    fn unpacks_offset_delta_pack_objects_as_loose_objects() {
        let objects_dir = temp_path("unpack-offset-delta").join("objects");
        let database = LooseObjectDb::new(&objects_dir);
        let pack = pack_with_offset_delta_blob();

        let object_ids = database
            .unpack_pack_as_loose(&pack)
            .expect("offset delta should unpack");

        let base_id = hash_object(ObjectKind::Blob, b"hello");
        let delta_id = hash_object(ObjectKind::Blob, b"hello!");
        assert_eq!(object_ids, [base_id, delta_id]);
        let object = database.read_object(delta_id).expect("delta object");
        assert_eq!(object.kind, ObjectKind::Blob);
        assert_eq!(object.data, b"hello!");
    }

    #[test]
    fn unpacks_ref_delta_pack_objects_as_loose_objects() {
        let objects_dir = temp_path("unpack-ref-delta").join("objects");
        let database = LooseObjectDb::new(&objects_dir);
        let base_id = database
            .write_object(ObjectKind::Blob, b"hello")
            .expect("base object");
        let pack = pack_with_ref_delta_blob(base_id);

        let object_ids = database
            .unpack_pack_as_loose(&pack)
            .expect("ref delta should unpack");

        let delta_id = hash_object(ObjectKind::Blob, b"hello!");
        assert_eq!(object_ids, [delta_id]);
        let object = database.read_object(delta_id).expect("delta object");
        assert_eq!(object.kind, ObjectKind::Blob);
        assert_eq!(object.data, b"hello!");
    }

    fn empty_pack() -> Vec<u8> {
        let mut pack = Vec::new();
        pack.extend_from_slice(b"PACK");
        pack.extend_from_slice(&2_u32.to_be_bytes());
        pack.extend_from_slice(&0_u32.to_be_bytes());
        let checksum = sha1_bytes(&pack);
        pack.extend_from_slice(&checksum);
        pack
    }

    fn pack_with_one_blob(data: &[u8]) -> Vec<u8> {
        let mut object = Vec::new();
        object.push(0x30 | data.len() as u8);
        object.extend_from_slice(&zlib(data));
        pack_with_objects(1, &object)
    }

    fn pack_with_offset_delta_blob() -> Vec<u8> {
        let mut base = Vec::new();
        base.push(0x35);
        base.extend_from_slice(&zlib(b"hello"));

        let base_offset = 12;
        let delta_offset = base_offset + base.len();
        let distance = delta_offset - base_offset;
        let delta = blob_delta_hello_to_hello_bang();
        let mut delta_object = Vec::new();
        delta_object.push(0x60 | delta.len() as u8);
        delta_object.push(distance as u8);
        delta_object.extend_from_slice(&zlib(&delta));

        let mut objects = Vec::new();
        objects.extend_from_slice(&base);
        objects.extend_from_slice(&delta_object);
        pack_with_objects(2, &objects)
    }

    fn pack_with_ref_delta_blob(base_id: crate::ObjectId) -> Vec<u8> {
        let delta = blob_delta_hello_to_hello_bang();
        let mut object = Vec::new();
        object.push(0x70 | delta.len() as u8);
        object.extend_from_slice(base_id.as_bytes());
        object.extend_from_slice(&zlib(&delta));
        pack_with_objects(1, &object)
    }

    fn blob_delta_hello_to_hello_bang() -> Vec<u8> {
        vec![5, 6, 0x90, 5, 1, b'!']
    }

    fn pack_with_objects(object_count: u32, objects: &[u8]) -> Vec<u8> {
        let mut pack = Vec::new();
        pack.extend_from_slice(b"PACK");
        pack.extend_from_slice(&2_u32.to_be_bytes());
        pack.extend_from_slice(&object_count.to_be_bytes());
        pack.extend_from_slice(objects);
        let checksum = sha1_bytes(&pack);
        pack.extend_from_slice(&checksum);
        pack
    }

    fn zlib(data: &[u8]) -> Vec<u8> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data).expect("compress data");
        encoder.finish().expect("finish zlib")
    }

    fn temp_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("rit-odb-{name}-{unique}"))
    }
}
