//! Explicit standard-handle inheritance for console-less owned workers.
use std::{
    fs::{File, OpenOptions},
    os::windows::io::AsRawHandle,
    path::Path,
    ptr::{null, null_mut},
};
use windows_sys::Win32::{Foundation::*, System::Threading::*};

#[derive(Clone, Copy)]
pub struct Redirection<'a> {
    pub input: Option<&'a Path>,
    pub log: Option<&'a Path>,
    pub max_log_bytes: u64,
}

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

pub struct StandardHandles {
    handles: [Handle; 3],
    // Allocation must outlive the initialized attribute-list pointer.
    _storage: Vec<usize>,
    attributes: LPPROC_THREAD_ATTRIBUTE_LIST,
    _inherited_list: Box<[HANDLE; 3]>,
}

fn inherited(file: &File) -> Result<Handle, String> {
    unsafe {
        let process = GetCurrentProcess();
        let mut duplicate = null_mut();
        if DuplicateHandle(
            process,
            file.as_raw_handle(),
            process,
            &mut duplicate,
            0,
            1,
            DUPLICATE_SAME_ACCESS,
        ) == 0
        {
            return Err("Cannot duplicate worker standard handle.".into());
        }
        Ok(Handle(duplicate))
    }
}

impl StandardHandles {
    pub fn create(io: Redirection<'_>) -> Result<Self, String> {
        let input = if let Some(path) = io.input {
            if !path.is_absolute() {
                return Err("Worker input must have an absolute path.".into());
            }
            let file = File::open(path).map_err(|_| "Cannot open worker text input.")?;
            let metadata = file
                .metadata()
                .map_err(|_| "Cannot inspect worker text input.")?;
            if !metadata.is_file() || metadata.len() > 128 * 1024 {
                return Err(
                    "Worker text input must be a regular file no larger than 128 KiB.".into(),
                );
            }
            file
        } else {
            File::open("NUL").map_err(|_| "Cannot open null worker input.")?
        };
        let output = if let Some(path) = io.log {
            if !path.is_absolute() || io.max_log_bytes == 0 || io.max_log_bytes > 8 * 1024 * 1024 {
                return Err("Invalid bounded worker log configuration.".into());
            }
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map_err(|_| "Cannot create worker log without overwriting an existing file.")?
        } else {
            OpenOptions::new()
                .write(true)
                .open("NUL")
                .map_err(|_| "Cannot open null worker output.")?
        };
        let handles = [inherited(&input)?, inherited(&output)?, inherited(&output)?];
        unsafe {
            let mut size = 0;
            InitializeProcThreadAttributeList(null_mut(), 1, 0, &mut size);
            if size == 0 || size > 16384 {
                return Err("Unexpected worker attribute-list size.".into());
            }
            let mut storage = vec![0usize; size.div_ceil(std::mem::size_of::<usize>())];
            let attributes = storage.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST;
            if InitializeProcThreadAttributeList(attributes, 1, 0, &mut size) == 0 {
                return Err("Cannot initialize worker handle whitelist.".into());
            }
            let inherited_list = Box::new([handles[0].0, handles[1].0, handles[2].0]);
            let owned = Self {
                handles,
                _storage: storage,
                attributes,
                _inherited_list: inherited_list,
            };
            if UpdateProcThreadAttribute(
                attributes,
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                owned._inherited_list.as_ptr() as _,
                std::mem::size_of_val(owned._inherited_list.as_ref()),
                null_mut(),
                null(),
            ) == 0
            {
                return Err("Cannot whitelist worker standard handles.".into());
            }
            Ok(owned)
        }
    }
    pub fn configure(&self, startup: &mut STARTUPINFOEXW) {
        startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.dwFlags |= STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = self.handles[0].0;
        startup.StartupInfo.hStdOutput = self.handles[1].0;
        startup.StartupInfo.hStdError = self.handles[2].0;
        startup.lpAttributeList = self.attributes;
    }
}
impl Drop for StandardHandles {
    fn drop(&mut self) {
        unsafe {
            DeleteProcThreadAttributeList(self.attributes);
        }
    }
}
