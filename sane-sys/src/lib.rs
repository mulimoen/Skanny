#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

pub const fn SANE_VERSION_MAJOR(code: SANE_Int) -> SANE_Word {
    (code >> 24) as SANE_Word & 0xff
}

pub const fn SANE_VERSION_MINOR(code: SANE_Int) -> SANE_Word {
    (code >> 16) as SANE_Word & 0xff
}

pub const fn SANE_VERSION_BUILD(code: SANE_Int) -> SANE_Word {
    (code >> 0) as SANE_Word & 0xffff
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smokescreen() {
        use std::ptr::null_mut;
        let status = unsafe { sane_init(null_mut(), None) };
        assert_eq!(status, SANE_Status_SANE_STATUS_GOOD);

        unsafe { sane_exit() }
    }
}
