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

    fn devices(&self, only_local: bool) -> Result<impl ExactSizeIterator<Item = Device>, Error> {
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

        Ok((0..num_devices).map(move |i| {
            let device = unsafe { *device_list.offset(i) };
            Device(device)
        }))
    }
}
impl Drop for Context {
    fn drop(&mut self) {
        unsafe { sane_exit() }
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
    fn open(&self) -> Result<Handle, Error> {
        let mut handle = std::ptr::null_mut();
        unsafe { checked(|| sane_open((*self.0).name, &mut handle))? };

        Ok(Handle(handle))
    }
}

struct Handle(SANE_Handle);

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { sane_close(self.0) }
    }
}

impl Handle {
    fn descriptors(&self) -> impl ExactSizeIterator<Item = Descriptor> + '_ {
        // Guaranteed to exist
        let first_desc = self.get_descriptor(0).unwrap();
        assert_eq!(first_desc.type_(), SANE_Value_Type_SANE_TYPE_INT);
        assert_eq!(first_desc.size(), std::mem::size_of::<SANE_Int>() as _);
        let mut num_desc: SANE_Int = 0;
        unsafe {
            checked(|| {
                sane_control_option(
                    self.0,
                    0,
                    SANE_Action_SANE_ACTION_GET_VALUE,
                    &mut num_desc as *mut _ as _,
                    std::ptr::null_mut(),
                )
            })
            .unwrap()
        };
        (0..num_desc).map(move |i| self.get_descriptor(i as _).unwrap())
    }
    fn get_descriptor(&self, index: usize) -> Option<Descriptor> {
        let desc = unsafe { sane_get_option_descriptor(self.0, index as _) };
        if desc.is_null() {
            None
        } else {
            Some(Descriptor(desc))
        }
    }
}

struct Descriptor(*const SANE_Option_Descriptor);

impl Descriptor {
    fn name(&self) -> &str {
        let cstr = unsafe { CStr::from_ptr((*self.0).name) };
        cstr.to_str().unwrap()
    }
    fn type_(&self) -> SANE_Value_Type {
        unsafe { (*self.0).type_ }
    }
    fn size(&self) -> SANE_Int {
        unsafe { (*self.0).size }
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
    let mut chosen_device = None;
    for device in context.devices(true).unwrap() {
        println!("Device:");
        let name = device.name();
        println!("\tname: {}", name);
        println!("\tvendor: {}", device.vendor());
        println!("\tmodel: {}", device.model());
        println!("\ttype: {}", device.type_());
        chosen_device = Some(device);
    }

    let device = chosen_device.unwrap();
    let handle = device.open().unwrap();

    println!("Options:");
    for descriptor in handle.descriptors() {
        println!("\t{}", descriptor.name());
    }
}
