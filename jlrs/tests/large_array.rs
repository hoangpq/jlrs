mod util;
#[cfg(feature = "sync-rt")]
#[cfg(not(all(target_os = "windows", feature = "lts")))]
mod tests {
    use jlrs::prelude::*;

    use super::util::JULIA;

    fn create_large_array() {
        JULIA.with(|j| {
            let mut frame = StackFrame::new();
            let mut jlrs = j.borrow_mut();

            jlrs.instance(&mut frame)
                .scope(|mut frame| {
                    let array = Array::new::<f32, _, _>(
                        frame.as_extended_target(),
                        &[1, 1, 1, 1, 1, 1, 1, 1, 1][..],
                    );
                    assert!(array.is_ok());
                    Ok(())
                })
                .unwrap();
        });
    }

    fn move_large_array() {
        JULIA.with(|j| {
            let mut frame = StackFrame::new();
            let mut jlrs = j.borrow_mut();

            jlrs.instance(&mut frame)
                .scope(|mut frame| {
                    let array = Array::from_vec(
                        frame.as_extended_target(),
                        vec![1u64],
                        &[1, 1, 1, 1, 1, 1, 1, 1, 1][..],
                    );
                    assert!(array.is_ok());
                    Ok(())
                })
                .unwrap();
        });
    }

    fn borrow_large_array() {
        JULIA.with(|j| {
            let mut frame = StackFrame::new();
            let mut jlrs = j.borrow_mut();

            jlrs.instance(&mut frame)
                .scope(|mut frame| {
                    let mut data = vec![1u32];
                    let array = {
                        Array::from_slice(
                            frame.as_extended_target(),
                            &mut data,
                            &[1, 1, 1, 1, 1, 1, 1, 1, 1][..],
                        )?
                        .into_jlrs_result()
                    };
                    assert!(array.is_ok());
                    Ok(())
                })
                .unwrap();
        });
    }

    fn create_large_typed_array() {
        JULIA.with(|j| {
            let mut frame = StackFrame::new();
            let mut jlrs = j.borrow_mut();

            jlrs.instance(&mut frame)
                .scope(|mut frame| {
                    let array = TypedArray::<f32>::new(
                        frame.as_extended_target(),
                        &[1, 1, 1, 1, 1, 1, 1, 1, 1][..],
                    );
                    assert!(array.is_ok());
                    Ok(())
                })
                .unwrap();
        });
    }

    fn move_large_typed_array() {
        JULIA.with(|j| {
            let mut frame = StackFrame::new();
            let mut jlrs = j.borrow_mut();

            jlrs.instance(&mut frame)
                .scope(|mut frame| {
                    let array = TypedArray::from_vec(
                        frame.as_extended_target(),
                        vec![1u64],
                        &[1, 1, 1, 1, 1, 1, 1, 1, 1][..],
                    );
                    assert!(array.is_ok());
                    Ok(())
                })
                .unwrap();
        });
    }

    fn borrow_large_typed_array() {
        JULIA.with(|j| {
            let mut frame = StackFrame::new();
            let mut jlrs = j.borrow_mut();

            jlrs.instance(&mut frame)
                .scope(|mut frame| {
                    let mut data = vec![1u32];
                    let array = {
                        TypedArray::from_slice(
                            frame.as_extended_target(),
                            &mut data,
                            &[1, 1, 1, 1, 1, 1, 1, 1, 1][..],
                        )?
                        .into_jlrs_result()
                    };
                    assert!(array.is_ok());
                    Ok(())
                })
                .unwrap();
        });
    }

    #[test]
    fn large_array_tests() {
        create_large_array();
        move_large_array();
        borrow_large_array();
        create_large_typed_array();
        move_large_typed_array();
        borrow_large_typed_array();
    }
}
