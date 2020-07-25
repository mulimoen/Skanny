#![allow(unused)]

use sane_sys::*;
use std::ffi::CStr;

use gumdrop::Options;

#[derive(Debug, Copy, Clone, PartialEq)]
enum Error {
    Status(SANE_Status),
    WrongType,
}

impl Error {
    fn is_eof(self) -> bool {
        self == Error::Status(SANE_Status_SANE_STATUS_EOF)
    }
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
            Error::WrongType => write!(f, "Expected another type here"),
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
    fn from_name(name: &str) -> Result<Self, Error> {
        let name = std::ffi::CString::new(name).unwrap();
        let mut handle = std::ptr::null_mut();
        unsafe { checked(|| sane_open(name.as_ptr(), &mut handle))? };
        Ok(Self(handle))
    }
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
        (1..num_desc).map(move |i| self.get_descriptor(i as _).unwrap())
    }
    fn get_descriptor(&self, index: usize) -> Option<Descriptor> {
        let desc = unsafe { sane_get_option_descriptor(self.0, index as _) };
        if desc.is_null() {
            None
        } else {
            Some(Descriptor(desc))
        }
    }
    fn options(&self) -> impl ExactSizeIterator<Item = Opt> + '_ {
        self.descriptors()
            .enumerate()
            .map(move |(index, descriptor)| Opt {
                handle: &self.0,
                index: index + 1, /* skipping first descriptor */
                descriptor,
            })
    }

    fn parameters(&self) -> Result<Parameters, Error> {
        let mut parameters = std::mem::MaybeUninit::uninit();
        unsafe { checked(|| sane_get_parameters(self.0, parameters.as_mut_ptr()))? }
        Ok(Parameters(unsafe { parameters.assume_init() }))
    }
    fn start(&self) -> Result<Acquisition, Error> {
        unsafe { checked(|| sane_start(self.0)) };
        Ok(Acquisition { handle: &self })
    }
}

#[derive(Debug)]
struct Descriptor(*const SANE_Option_Descriptor);

impl Descriptor {
    fn name(&self) -> &str {
        let name = unsafe { (*self.0).name };
        if name.is_null() {
            ""
        } else {
            let cstr = unsafe { CStr::from_ptr(name) };
            cstr.to_str().unwrap()
        }
    }
    fn desc(&self) -> &str {
        let desc = unsafe { (*self.0).desc };
        if desc.is_null() {
            ""
        } else {
            let cstr = unsafe { CStr::from_ptr(desc) };
            cstr.to_str().unwrap()
        }
    }
    fn type_(&self) -> SANE_Value_Type {
        unsafe { (*self.0).type_ }
    }
    fn size(&self) -> SANE_Int {
        unsafe { (*self.0).size }
    }
}

#[derive(Debug, Clone)]
struct Parameters(SANE_Parameters);

impl Parameters {
    fn format(&self) -> SANE_Frame {
        self.0.format
    }
    fn last_frame(&self) -> SANE_Bool {
        self.0.last_frame
    }
    fn bytes_per_line(&self) -> SANE_Int {
        self.0.bytes_per_line
    }
    fn pixels_per_line(&self) -> SANE_Int {
        self.0.pixels_per_line
    }
    fn lines(&self) -> SANE_Int {
        self.0.lines
    }
    fn depth(&self) -> SANE_Int {
        self.0.depth
    }
}

#[derive(Debug)]
struct Opt {
    handle: *const SANE_Handle,
    descriptor: Descriptor,
    index: usize,
}

impl Opt {
    fn name(&self) -> &str {
        self.descriptor.name()
    }
    fn desc(&self) -> &str {
        self.descriptor.desc()
    }
    fn string_constraints(&self) -> Result<impl ExactSizeIterator<Item = &str>, Error> {
        #[allow(non_upper_case_globals)]
        match unsafe { (*self.descriptor.0) }.constraint_type {
            SANE_Constraint_Type_SANE_CONSTRAINT_STRING_LIST => (),
            typ => panic!("type {} is not a string constraint", typ),
        }
        let mut len = 0;
        let mut walker = unsafe { { *self.descriptor.0 }.constraint.string_list };
        unsafe {
            while !(*walker).is_null() {
                len += 1;
                walker = walker.offset(1);
            }
        }
        Ok((0..len).map(move |i| unsafe {
            let list = (*self.descriptor.0).constraint.string_list;
            let cstr = std::ffi::CStr::from_ptr(*list.offset(i) as _);
            cstr.to_str().unwrap()
        }))
    }
    fn get_string(&self) -> Result<String, Error> {
        if self.descriptor.type_() != SANE_Value_Type_SANE_TYPE_STRING {
            return Err(Error::WrongType);
        }
        let mut val: Vec<u8> = vec![0; self.descriptor.size() as _];
        unsafe {
            checked(|| {
                sane_control_option(
                    *self.handle,
                    self.index as i32,
                    SANE_Action_SANE_ACTION_GET_VALUE,
                    val.as_mut_ptr() as *mut _,
                    std::ptr::null_mut(),
                )
            })?;
        }
        let first_zero = val.iter().position(|&x| x == 0).unwrap_or(val.len());
        val.resize(first_zero, 0);
        Ok(String::from_utf8(val).unwrap())
    }
    fn set_string(&self, val: &str) -> Result<(), Error> {
        assert_eq!(self.descriptor.type_(), SANE_Value_Type_SANE_TYPE_STRING);

        let mut val = val.as_bytes().to_vec();
        val.push(0);

        let mut info = 0;
        unsafe {
            checked(|| {
                sane_control_option(
                    *self.handle,
                    self.index as _,
                    SANE_Action_SANE_ACTION_SET_VALUE,
                    val.as_mut_ptr() as *mut _,
                    &mut info,
                )
            })?;
        };

        Ok(())
    }
    fn int_constraints(&self) -> Result<&[SANE_Word], Error> {
        #[allow(non_upper_case_globals)]
        match unsafe { (*self.descriptor.0) }.constraint_type {
            SANE_Constraint_Type_SANE_CONSTRAINT_WORD_LIST => (),
            typ => panic!("type {} is not a word constraint", typ),
        }
        let list = unsafe { (*self.descriptor.0).constraint.word_list };
        assert!(!list.is_null());
        let len = unsafe { *list };
        let list = unsafe { std::slice::from_raw_parts(list, len as usize + 1) };
        Ok(&list[1..])
    }
    fn get_int(&self) -> Result<SANE_Int, Error> {
        assert!(
            self.descriptor.type_() == SANE_Value_Type_SANE_TYPE_INT
                || self.descriptor.type_() == SANE_Value_Type_SANE_TYPE_FIXED
        );
        assert_eq!(self.descriptor.size(), std::mem::size_of::<SANE_Int>() as _);
        let mut val = 0;
        unsafe {
            checked(|| {
                sane_control_option(
                    *self.handle,
                    self.index as i32,
                    SANE_Action_SANE_ACTION_GET_VALUE,
                    &mut val as *mut _ as _,
                    std::ptr::null_mut(),
                )
            })?;
        }
        Ok(val)
    }
    fn set_int(&self, val: &mut i32) -> Result<(), Error> {
        assert!(
            self.descriptor.type_() == SANE_Value_Type_SANE_TYPE_INT
                || self.descriptor.type_() == SANE_Value_Type_SANE_TYPE_FIXED
        );
        assert_eq!(self.descriptor.size(), std::mem::size_of::<SANE_Int>() as _);
        unsafe {
            checked(|| {
                sane_control_option(
                    *self.handle,
                    self.index as i32,
                    SANE_Action_SANE_ACTION_SET_VALUE,
                    val as *mut _ as _,
                    std::ptr::null_mut(),
                )
            })?;
        }
        Ok(())
    }
    fn get_range(&self) -> Result<Range, Error> {
        #[allow(non_upper_case_globals)]
        match unsafe { (*self.descriptor.0) }.constraint_type {
            SANE_Constraint_Type_SANE_CONSTRAINT_RANGE => (),
            typ => return Err(Error::WrongType),
        }
        let range = unsafe { *(*self.descriptor.0).constraint.range };
        Ok(Range(range))
    }
}

#[derive(Debug, Copy, Clone)]
struct Range(SANE_Range);

impl Range {
    fn min(&self) -> SANE_Word {
        self.0.min
    }
    fn max(&self) -> SANE_Word {
        self.0.max
    }
    fn quant(&self) -> SANE_Word {
        self.0.quant
    }
}

struct Acquisition<'a> {
    handle: &'a Handle,
}

impl<'a> Acquisition<'a> {
    fn cancel(self) {}
    fn restart(&self) -> Result<(), Error> {
        self.handle.start().map(|x| std::mem::forget(x))
    }

    fn read_image(&self, mut buffer: &mut [u8]) -> Result<(), Error> {
        unsafe {
            'read_loop: loop {
                let mut len = 0;
                let e = checked(|| {
                    sane_read(
                        self.handle.0,
                        buffer.as_mut_ptr(),
                        buffer.len() as _,
                        &mut len,
                    )
                });
                buffer = &mut buffer[len as usize..];
                if let Err(err) = e {
                    if err.is_eof() {
                        break 'read_loop;
                    } else {
                        return Err(err);
                    }
                }
                // std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
        assert_eq!(buffer.len(), 0);
        Ok(())
    }

    fn get_image(self) -> Result<Image, Error> {
        let parameters = self.handle.parameters()?;

        let bytesize = parameters.pixels_per_line()
            * (parameters.depth() / 8)
            * parameters.lines()
            * if parameters.format() == SANE_Frame_SANE_FRAME_GRAY {
                1
            } else {
                3
            };
        let mut image = vec![0_u8; bytesize as _];

        // unsafe { checked(|| sane_set_io_mode(self.handle.0, SANE_FALSE as _))? };
        #[allow(non_upper_case_globals)]
        match parameters.format() {
            SANE_Frame_SANE_FRAME_GRAY => self.read_image(&mut image[..])?,
            SANE_Frame_SANE_FRAME_RGB => {
                self.read_image(&mut image[..])?;
            }
            format => todo!("format: {}", format),
        };

        #[allow(non_upper_case_globals)]
        match (parameters.format(), parameters.depth()) {
            (SANE_Frame_SANE_FRAME_GRAY, 8) => Ok(Image::Gray8(
                image::ImageBuffer::from_raw(
                    parameters.pixels_per_line() as _,
                    parameters.lines() as _,
                    image,
                )
                .unwrap(),
            )),
            (SANE_Frame_SANE_FRAME_RGB, 8) => Ok(Image::Rgb8(
                image::ImageBuffer::from_raw(
                    parameters.pixels_per_line() as _,
                    parameters.lines() as _,
                    image,
                )
                .unwrap(),
            )),
            (format, depth) => unimplemented!("format: {} depth: {}", format, depth),
        }
    }
}

impl Drop for Acquisition<'_> {
    fn drop(&mut self) {
        unsafe { sane_cancel(self.handle.0) }
    }
}

enum Image {
    Rgb8(image::ImageBuffer<image::Rgb<u8>, Vec<u8>>),
    Gray8(image::ImageBuffer<image::Luma<u8>, Vec<u8>>),
}

impl Image {
    fn save(&self, path: impl AsRef<std::path::Path>) -> image::ImageResult<()> {
        match self {
            Image::Gray8(im) => im.save(path),
            Image::Rgb8(im) => im.save(path),
        }
    }
}

#[derive(Debug, Options)]
struct CliOptions {
    #[options(help = "Use a destdevice")]
    testdevice: bool,
    #[options(help = "Directory to store images")]
    dir: Option<String>,
}

fn main() {
    let cliopts = CliOptions::parse_args_default_or_exit();

    let (context, version) = Context::init().unwrap();
    println!(
        "Version: major: {} minor: {} build: {}",
        version.major(),
        version.minor(),
        version.build()
    );
    let handle = if cliopts.testdevice {
        Handle::from_name("test").unwrap()
    } else {
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
        device.open().unwrap()
    };

    println!("Options:");
    for option in handle.options() {
        let optname = option.name();
        if optname.is_empty() {
            continue;
        }
        println!("\t{}", optname);
        for line in option.desc().lines() {
            println!("\t\t{}", line);
        }
        match optname {
            "mode" => {
                if !cliopts.testdevice {
                    option.set_string("Color").unwrap()
                }
                let active_mode = option.get_string().unwrap();
                print!("\t\t");
                for opt in option.string_constraints().unwrap() {
                    if opt == active_mode {
                        print!("[{}]\t", opt);
                    } else {
                        print!("{}\t", opt);
                    }
                }
                println!();
                option.set_string("color").unwrap()
            }
            "depth" => {
                println!("\t\tCurrent depth: {}", option.get_int().unwrap());
            }
            "resolution" => {
                if let Ok(range) = option.get_range() {
                    let active_resolution = option.get_int().unwrap();
                    println!(
                        "\t\tmin:{} max:{} quant:{} :: current: {}",
                        range.min(),
                        range.max(),
                        range.quant(),
                        active_resolution
                    );
                } else {
                    option.set_int(&mut 300).unwrap();
                    let active_resolution = option.get_int().unwrap();
                    let resolutions = option.int_constraints().unwrap();
                    print!("\t\t");
                    for &res in resolutions {
                        if res == active_resolution {
                            print!("[{}]\t", res);
                        } else {
                            print!("{}\t", res);
                        }
                    }
                    println!();
                }
            }
            "test-picture" => {
                option.set_string("Color pattern");
            }
            _ => {}
        }
    }

    if let Some(dir) = cliopts.dir.as_ref() {
        let dir = std::path::Path::new(dir);
        std::fs::create_dir_all(&dir).unwrap();
        loop {
            // Check button
            let acq = handle.start().unwrap();
            let image = acq.get_image().unwrap();

            let now = std::time::SystemTime::now();
            let since_unix = now.duration_since(std::time::UNIX_EPOCH).unwrap();

            let mut imagepath = dir.join(format!(
                "plate_{}:{}.png",
                since_unix.as_secs(),
                since_unix.subsec_millis()
            ));
            assert!(!imagepath.exists());

            image.save(imagepath).unwrap();
        }
    } else {
        let acq = handle.start().unwrap();
        let image = acq.get_image().unwrap();
        image.save("test.png").unwrap();
    }
}
