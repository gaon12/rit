use super::super::{Credential, CredentialKind, SecretString};
use crate::{Result, RitError};
use std::ffi::c_void;
use std::ptr;

const CRED_TYPE_GENERIC: u32 = 1;
const CRED_PERSIST_LOCAL_MACHINE: u32 = 2;
const ERROR_NOT_FOUND: i32 = 1168;

#[repr(C)]
struct FileTime {
    low_date_time: u32,
    high_date_time: u32,
}

#[repr(C)]
struct CredentialW {
    flags: u32,
    credential_type: u32,
    target_name: *mut u16,
    comment: *mut u16,
    last_written: FileTime,
    credential_blob_size: u32,
    credential_blob: *mut u8,
    persist: u32,
    attribute_count: u32,
    attributes: *mut c_void,
    target_alias: *mut u16,
    user_name: *mut u16,
}

#[link(name = "Advapi32")]
unsafe extern "system" {
    fn CredReadW(
        target_name: *const u16,
        credential_type: u32,
        flags: u32,
        credential: *mut *mut CredentialW,
    ) -> i32;
    fn CredWriteW(credential: *const CredentialW, flags: u32) -> i32;
    fn CredDeleteW(target_name: *const u16, credential_type: u32, flags: u32) -> i32;
    fn CredFree(buffer: *mut c_void);
}

pub fn read(target: &str) -> Result<Option<Credential>> {
    let target = nul_terminated_utf16(target);
    let mut raw_credential = ptr::null_mut();
    let ok = unsafe {
        // SAFETY: `target` is NUL-terminated and lives for the call. The API
        // initializes `raw_credential` on success, and we release it with
        // `CredFree` before returning.
        CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut raw_credential)
    };

    if ok == 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(ERROR_NOT_FOUND) {
            return Ok(None);
        }
        return Err(RitError::transport_io("windows credential manager", error));
    }

    let credential = unsafe {
        // SAFETY: `CredReadW` returned success, so `raw_credential` points to a
        // valid `CREDENTIALW` allocation until `CredFree`.
        credential_from_raw(&*raw_credential)
    };
    unsafe {
        // SAFETY: The pointer came from `CredReadW` and has not been freed.
        CredFree(raw_credential.cast());
    }
    Ok(Some(credential))
}

pub fn store(target: &str, credential: &Credential) -> Result<()> {
    let mut target = nul_terminated_utf16(target);
    let mut username = credential
        .username
        .as_ref()
        .map(|name| nul_terminated_utf16(name));
    let mut secret = credential.secret.expose_secret().as_bytes().to_vec();
    let blob_size = u32::try_from(secret.len()).map_err(|_| {
        RitError::invalid_input("credential secret is too large for Windows Credential Manager")
    })?;

    let record = CredentialW {
        flags: 0,
        credential_type: CRED_TYPE_GENERIC,
        target_name: target.as_mut_ptr(),
        comment: ptr::null_mut(),
        last_written: FileTime {
            low_date_time: 0,
            high_date_time: 0,
        },
        credential_blob_size: blob_size,
        credential_blob: secret.as_mut_ptr(),
        persist: CRED_PERSIST_LOCAL_MACHINE,
        attribute_count: 0,
        attributes: ptr::null_mut(),
        target_alias: ptr::null_mut(),
        user_name: username
            .as_mut()
            .map_or(ptr::null_mut(), |name| name.as_mut_ptr()),
    };

    let ok = unsafe {
        // SAFETY: All pointers in `record` point to live buffers for the
        // duration of this call, and sizes match the referenced buffers.
        CredWriteW(&record, 0)
    };
    if ok == 0 {
        return Err(RitError::transport_io(
            "windows credential manager",
            std::io::Error::last_os_error(),
        ));
    }

    Ok(())
}

pub fn erase(target: &str) -> Result<()> {
    let target = nul_terminated_utf16(target);
    let ok = unsafe {
        // SAFETY: `target` is NUL-terminated and lives for the call.
        CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0)
    };
    if ok == 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(ERROR_NOT_FOUND) {
            return Ok(());
        }
        return Err(RitError::transport_io("windows credential manager", error));
    }
    Ok(())
}

unsafe fn credential_from_raw(raw: &CredentialW) -> Credential {
    let username = unsafe {
        // SAFETY: `raw.user_name` is supplied by Credential Manager and is
        // valid for the lifetime of `raw`.
        wide_ptr_to_string(raw.user_name)
    };
    let secret = unsafe {
        // SAFETY: `raw.credential_blob` and `credential_blob_size` come from a
        // successful `CredReadW` allocation.
        std::slice::from_raw_parts(raw.credential_blob, raw.credential_blob_size as usize)
    };
    let secret = String::from_utf8_lossy(secret).into_owned();

    match username {
        Some(username) if !username.is_empty() => Credential {
            username: Some(username),
            secret: SecretString::new(secret),
            kind: CredentialKind::Password,
        },
        _ => Credential {
            username: None,
            secret: SecretString::new(secret),
            kind: CredentialKind::Token,
        },
    }
}

unsafe fn wide_ptr_to_string(value: *const u16) -> Option<String> {
    if value.is_null() {
        return None;
    }

    let mut length = 0;
    while unsafe {
        // SAFETY: The caller guarantees `value` points to a NUL-terminated
        // UTF-16 string returned by Windows Credential Manager.
        *value.add(length)
    } != 0
    {
        length += 1;
    }
    let units = unsafe {
        // SAFETY: The loop above found the NUL terminator, so `length` UTF-16
        // units are initialized and readable.
        std::slice::from_raw_parts(value, length)
    };
    Some(String::from_utf16_lossy(units))
}

fn nul_terminated_utf16(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
