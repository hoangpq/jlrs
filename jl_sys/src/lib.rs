#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

//! The documentation found on docs.rs corresponds to Julia version 1.7.0.

use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::{null_mut, NonNull};
use std::sync::atomic::{AtomicPtr, Ordering};

#[cfg(all(not(feature = "use-bindgen"), target_os = "linux"))]
pub mod bindings;

#[cfg(all(not(feature = "use-bindgen"), target_os = "linux"))]
pub use bindings::*;

#[cfg(target_os = "windows")]
pub mod bindings_win;

#[cfg(target_os = "windows")]
pub use bindings_win::*;

#[cfg(all(feature = "use-bindgen", target_os = "linux"))]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[inline(always)]
fn llt_align(x: usize, sz: usize) -> usize {
    (x + sz - 1) & !(sz - 1)
}

#[inline(always)]
pub unsafe fn jl_astaggedvalue(v: *mut jl_value_t) -> *mut jl_taggedvalue_t {
    let v_usize = v as usize;
    let sz = size_of::<jl_taggedvalue_t>();

    (v_usize - sz) as *mut jl_taggedvalue_t
}

#[inline(always)]
pub unsafe fn jl_typeof(v: *mut jl_value_t) -> *mut jl_value_t {
    ((*jl_astaggedvalue(v)).__bindgen_anon_1.header as usize & !15usize) as *mut jl_value_t
}

#[inline(always)]
pub unsafe fn jl_svec_data(t: *mut jl_svec_t) -> *mut *mut jl_value_t {
    t.cast::<u8>().add(size_of::<jl_svec_t>()).cast()
}

#[inline(always)]
pub unsafe fn jl_array_data(array: *mut jl_value_t) -> *mut c_void {
    NonNull::new_unchecked(array)
        .cast::<jl_array_t>()
        .as_ref()
        .data
        .cast()
}

#[inline(always)]
pub unsafe fn jl_array_ndims(array: *mut jl_array_t) -> u16 {
    NonNull::new_unchecked(array).as_ref().flags.ndims()
}

#[inline(always)]
pub unsafe fn jl_array_data_owner(a: *mut jl_array_t) -> *mut jl_value_t {
    a.cast::<u8>()
        .add(jlrs_array_data_owner_offset(jl_array_ndims(a)) as usize)
        .cast::<jl_value_t>()
}

#[inline(always)]
pub unsafe fn jl_get_fieldtypes(st: *mut jl_datatype_t) -> *mut jl_svec_t {
    let tys = NonNull::new_unchecked(st).as_ref().types;
    if tys.is_null() {
        jl_compute_fieldtypes(st, null_mut())
    } else {
        tys
    }
}

#[inline(always)]
pub unsafe fn jl_dt_layout_fields(d: *const u8) -> *const u8 {
    d.add(size_of::<jl_datatype_layout_t>())
}

#[inline(always)]
pub unsafe fn jl_array_ndimwords(ndims: u32) -> i32 {
    if ndims < 3 {
        0
    } else {
        ndims as i32 - 2
    }
}

#[inline(always)]
pub unsafe fn jl_gc_wb(parent: *mut jl_value_t, ptr: *mut jl_value_t) {
    let parent_tagged = NonNull::new_unchecked(jl_astaggedvalue(parent)).as_ref();
    let ptr = NonNull::new_unchecked(jl_astaggedvalue(ptr)).as_ref();

    if parent_tagged.__bindgen_anon_1.bits.gc() == 3 && (ptr.__bindgen_anon_1.bits.gc() & 1) == 0 {
        jl_gc_queue_root(parent)
    }
}

#[inline(always)]
pub unsafe fn jl_symbol_name_(s: *mut jl_sym_t) -> *mut u8 {
    s.cast::<u8>()
        .add(llt_align(size_of::<jl_sym_t>(), size_of::<*mut c_void>()))
}

#[inline(always)]
pub unsafe fn jl_fielddesc_size(fielddesc_type: i8) -> u32 {
    2 << fielddesc_type
}

#[inline(always)]
pub unsafe fn jl_field_isptr(st: *mut jl_datatype_t, i: i32) -> bool {
    let ly = NonNull::new_unchecked(
        NonNull::new_unchecked(st).as_ref().layout as *mut jl_datatype_layout_t,
    )
    .as_ref();
    assert!(i >= 0 && (i as u32) < ly.nfields);
    NonNull::new_unchecked(
        jl_dt_layout_fields(ly as *const _ as *mut u8)
            .add(jl_fielddesc_size(ly.fielddesc_type() as i8) as usize * i as usize)
            as *mut jl_fielddesc8_t,
    )
    .as_ref()
    .isptr()
        != 0
}

#[inline(always)]
pub unsafe fn jl_field_size(st: *mut jl_datatype_t, i: isize) -> u32 {
    let ly = NonNull::new_unchecked(
        NonNull::new_unchecked(st).as_ref().layout as *mut jl_datatype_layout_t,
    )
    .as_ref();
    assert!(i >= 0 && (i as u32) < ly.nfields);
    match ly.fielddesc_type() {
        0 => (&*(jl_dt_layout_fields((ly as *const jl_datatype_layout_t).cast())
            .cast::<jl_fielddesc8_t>()
            .offset(i)))
            .size() as u32,
        1 => (&*(jl_dt_layout_fields((ly as *const jl_datatype_layout_t).cast())
            .cast::<jl_fielddesc16_t>()
            .offset(i)))
            .size() as u32,
        _ => (&*(jl_dt_layout_fields((ly as *const jl_datatype_layout_t).cast())
            .cast::<jl_fielddesc32_t>()
            .offset(i)))
            .size(),
    }
}

#[inline(always)]
pub unsafe fn jl_field_offset(st: *mut jl_datatype_t, i: isize) -> u32 {
    let ly = &*(&*st).layout;
    assert!(i >= 0 && (i as u32) < ly.nfields);
    match ly.fielddesc_type() {
        0 => {
            (&*(jl_dt_layout_fields((ly as *const jl_datatype_layout_t).cast())
                .cast::<jl_fielddesc8_t>()
                .offset(i)))
                .offset as u32
        }
        1 => {
            (&*(jl_dt_layout_fields((ly as *const jl_datatype_layout_t).cast())
                .cast::<jl_fielddesc16_t>()
                .offset(i)))
                .offset as u32
        }
        _ => {
            (&*(jl_dt_layout_fields((ly as *const jl_datatype_layout_t).cast())
                .cast::<jl_fielddesc32_t>()
                .offset(i)))
                .offset
        }
    }
}

#[inline(always)]
pub unsafe fn jl_array_dims_ptr<'a>(array: *mut jl_array_t) -> *mut usize {
    &mut NonNull::new_unchecked(array).as_mut().nrows
}

#[inline(always)]
pub unsafe fn jl_array_ptr_set(a: *mut jl_array_t, i: usize, x: *mut c_void) -> *mut jl_value_t {
    assert!(NonNull::new_unchecked(a).as_ref().flags.ptrarray() != 0);
    let a_data: *mut AtomicPtr<jl_value_t> = jl_array_data(a.cast()).cast();

    NonNull::new_unchecked(a_data.add(i))
        .as_ref()
        .store(x.cast(), Ordering::Relaxed);

    if !x.is_null() {
        if NonNull::new_unchecked(a).as_ref().flags.how() == 3 {
            jl_gc_wb(jl_array_data_owner(a).cast(), x.cast());
        } else {
            jl_gc_wb(a.cast(), x.cast());
        }
    }

    x.cast()
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;

    use super::*;

    #[test]
    fn sanity() {
        unsafe {
            jl_init();
            assert!(jl_is_initialized() != 0);
            let cmd = CString::new("sqrt(2.0)").unwrap();
            jl_eval_string(cmd.as_ptr());
            assert!(jl_exception_occurred().is_null());
            jl_atexit_hook(0);
        }
    }
}
