mod util;

#[cfg(feature = "sync-rt")]
mod tests {
    use jlrs::prelude::*;

    use crate::util::JULIA;

    fn extend_lifetime() {
        JULIA.with(|j| {
            let mut frame = StackFrame::new();
            let mut jlrs = j.borrow_mut();
            jlrs.instance(&mut frame)
                .scope(|mut frame| {
                    let output = frame.output();

                    frame
                        .scope(|frame| {
                            let func =
                                unsafe { Module::base(&frame).function(&frame, "+")?.wrapper() };
                            Ok(func.root(output))
                        })
                        .unwrap();

                    Ok(())
                })
                .unwrap();
        })
    }

    fn has_datatype() {
        JULIA.with(|j| {
            let mut frame = StackFrame::new();
            let mut jlrs = j.borrow_mut();
            jlrs.instance(&mut frame)
                .scope(|frame| {
                    let func_ty = unsafe {
                        Module::base(&frame)
                            .function(&frame, "+")?
                            .wrapper()
                            .datatype()
                    };

                    assert_eq!(func_ty.name(), "#+");

                    Ok(())
                })
                .unwrap();
        })
    }

    #[test]
    fn function_tests() {
        extend_lifetime();
        has_datatype();
    }
}
