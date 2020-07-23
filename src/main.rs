use sane_sys::*;
use std::ffi::CStr;

#[derive(Debug, Copy, Clone, PartialEq)]
enum Error {
    Status(SANE_Status),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[allow(non_upper_case_globals)]
        match *self {
            Error::Status(status) => match status {
                SANE_Status_SANE_STATUS_GOOD => write!(f, "No error"),
                SANE_Status_SANE_STATUS_UNSUPPORTED => write!(f, "Unsupported"),
                SANE_Status_SANE_STATUS_CANCELLED => write!(f, "Cancelled"),
                SANE_Status_SANE_STATUS_DEVICE_BUSY => write!(f, "Device busy"),
                SANE_Status_SANE_STATUS_INVAL => write!(f, "Invalid value"),
                SANE_Status_SANE_STATUS_EOF => write!(f, "End of file"),
                SANE_Status_SANE_STATUS_JAMMED => write!(f, "Document feeder is jammed"),
                SANE_Status_SANE_STATUS_NO_DOCS => write!(f, "Document feed is empty"),
                SANE_Status_SANE_STATUS_COVER_OPEN => write!(f, "Cover is open"),
                SANE_Status_SANE_STATUS_IO_ERROR => write!(f, "Device IO failed"),
                SANE_Status_SANE_STATUS_NO_MEM => write!(f, "Not enough memory available"),
                SANE_Status_SANE_STATUS_ACCESS_DENIED => write!(f, "Access denied"),
                _ => write!(f, "UNKNOWN ERROR: {}", status),
            },
        }
    }
}

impl std::error::Error for Error {}

fn checked(f: impl FnOnce() -> SANE_Status) -> Result<(), Error> {
    let status = f();
    if status != SANE_Status_SANE_STATUS_GOOD {
        Err(Error::Status(status))
    } else {
        Ok(())
    }
}

/// Must be kept active during the scan session
struct Context {}
impl Context {
    fn init() -> Result<(Self, Version), Error> {
        let mut version_code = -1;
        unsafe {
            checked(|| sane_init(&mut version_code, None))?;
        };
        Ok((Context {}, Version(version_code)))
    }

    fn devices(&self, only_local: bool) -> Result<impl Iterator<Item = Device>, Error> {
        let mut device_list: *mut *const SANE_Device = std::ptr::null_mut();
        unsafe {
            checked(|| sane_get_devices(&mut device_list, only_local as _))?;
        }

        let mut num_devices = 0;
        unsafe {
            let mut traveller = device_list;
            while !(*traveller).is_null() {
                traveller = traveller.offset(1);
                num_devices += 1;
            }
        }
        println!("{}", num_devices);

        Ok((0..num_devices).map(move |i| {
            let device = unsafe { *device_list.offset(i) };
            Device(device)
        }))
    }
}

struct Device(*const SANE_Device);

impl Device {
    fn name(&self) -> &str {
        let cstr = unsafe { CStr::from_ptr((*self.0).name) };
        cstr.to_str().unwrap()
    }
    fn vendor(&self) -> &str {
        let cstr = unsafe { CStr::from_ptr((*self.0).vendor) };
        cstr.to_str().unwrap()
    }
    fn model(&self) -> &str {
        let cstr = unsafe { CStr::from_ptr((*self.0).model) };
        cstr.to_str().unwrap()
    }
    fn type_(&self) -> &str {
        let cstr = unsafe { CStr::from_ptr((*self.0).type_) };
        cstr.to_str().unwrap()
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe { sane_exit() }
    }
}

#[derive(Copy, Clone)]
#[repr(transparent)]
struct Version(SANE_Int);

impl Version {
    fn major(self) -> SANE_Word {
        SANE_VERSION_MAJOR(self.0)
    }
    fn minor(self) -> SANE_Word {
        SANE_VERSION_MINOR(self.0)
    }
    fn build(self) -> SANE_Word {
        SANE_VERSION_BUILD(self.0)
    }
}

fn main() {
    let (context, version) = Context::init().unwrap();
    println!(
        "Version: major: {} minor: {} build: {}",
        version.major(),
        version.minor(),
        version.build()
    );
    for device in context.devices(true).unwrap() {
        println!("Device:");
        println!("\tname: {}", device.name());
        println!("\tvendor: {}", device.vendor());
        println!("\tmodel: {}", device.model());
        println!("\ttype: {}", device.type_());
    }
}
