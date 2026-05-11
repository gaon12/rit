use flate2::{Compression, write::ZlibEncoder};
use std::io::Write;

use crate::{ObjectId, ObjectKind, Result, RitError, object::sha1_bytes};

use super::LooseObjectDb;

impl LooseObjectDb {
    /// Builds a Git packfile containing whole objects in the requested order.
    pub fn build_pack_from_objects(&self, object_ids: &[ObjectId]) -> Result<Vec<u8>> {
        let mut pack = Vec::new();
        pack.extend_from_slice(b"PACK");
        pack.extend_from_slice(&2_u32.to_be_bytes());
        pack.extend_from_slice(&(object_ids.len() as u32).to_be_bytes());

        for object_id in object_ids {
            let object = self.read_object(*object_id)?;
            write_pack_object_header(&mut pack, object.kind, object.data.len());
            let compressed = zlib_compress(&object.data)?;
            pack.extend_from_slice(&compressed);
        }

        let checksum = sha1_bytes(&pack);
        pack.extend_from_slice(&checksum);
        Ok(pack)
    }
}

fn write_pack_object_header(output: &mut Vec<u8>, kind: ObjectKind, size: usize) {
    let type_code = pack_type_code(kind);
    let mut remaining_size = size;
    let mut first = (type_code << 4) | ((remaining_size & 0x0f) as u8);
    remaining_size >>= 4;
    if remaining_size != 0 {
        first |= 0x80;
    }
    output.push(first);

    while remaining_size != 0 {
        let mut byte = (remaining_size & 0x7f) as u8;
        remaining_size >>= 7;
        if remaining_size != 0 {
            byte |= 0x80;
        }
        output.push(byte);
    }
}

fn pack_type_code(kind: ObjectKind) -> u8 {
    match kind {
        ObjectKind::Commit => 1,
        ObjectKind::Tree => 2,
        ObjectKind::Blob => 3,
        ObjectKind::Tag => 4,
    }
}

fn zlib_compress(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(data)
        .map_err(|source| RitError::io("pack-object", source))?;
    encoder
        .finish()
        .map_err(|source| RitError::io("pack-object", source))
}

#[cfg(test)]
mod tests {
    use super::LooseObjectDb;
    use crate::{ObjectKind, hash_object};
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn builds_pack_from_whole_objects() {
        let source = LooseObjectDb::new(temp_path("source").join("objects"));
        let target = LooseObjectDb::new(temp_path("target").join("objects"));
        let object_id = source
            .write_object(ObjectKind::Blob, b"hello")
            .expect("source object");

        let pack = source
            .build_pack_from_objects(&[object_id])
            .expect("pack should build");
        let ingested = target.ingest_pack(&pack).expect("pack should ingest");

        assert_eq!(ingested.object_ids, [object_id]);
        assert_eq!(
            target.read_object(object_id).expect("target object").data,
            b"hello"
        );

        remove_dir_all(source.objects_dir.parent().expect("source temp"));
        remove_dir_all(target.objects_dir.parent().expect("target temp"));
    }

    #[test]
    fn builds_empty_pack() {
        let database = LooseObjectDb::new(temp_path("empty").join("objects"));

        let pack = database
            .build_pack_from_objects(&[])
            .expect("empty pack should build");
        let ingested = database.ingest_pack(&pack).expect("empty pack ingests");

        assert!(ingested.object_ids.is_empty());
        remove_dir_all(database.objects_dir.parent().expect("database temp"));
    }

    #[test]
    fn rejects_missing_pack_objects() {
        let database = LooseObjectDb::new(temp_path("missing").join("objects"));
        let missing = hash_object(ObjectKind::Blob, b"missing");

        let error = database
            .build_pack_from_objects(&[missing])
            .expect_err("missing object should fail");

        assert!(error.to_string().contains(&missing.to_string()));
        remove_dir_all(database.objects_dir.parent().expect("database temp"));
    }

    fn temp_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("rit-pack-write-{name}-{unique}"))
    }

    fn remove_dir_all(path: &Path) {
        if path.exists() {
            fs::remove_dir_all(path).expect("temporary directory should be removed");
        }
    }
}
