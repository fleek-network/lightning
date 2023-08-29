use std::marker::PhantomData;

use lightning_interfaces::infu_collection::Collection;

pub struct ServiceExecutor<C: Collection> {
    collection: PhantomData<C>,
}

#[test]
fn x() {
    unsafe {
        let lib =
            libloading::Library::new("../../target/release/libfleek_service_ping_example.dylib")
                .unwrap();
        let func: libloading::Symbol<unsafe extern "C" fn() -> u32> = lib.get(b"my_func").unwrap();
        println!("{}", func());
    }
}
