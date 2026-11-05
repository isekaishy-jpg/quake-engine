use engine_host::NativeWindowRef;

fn require_send<T: Send>() {}

fn main() {
    require_send::<NativeWindowRef<'static>>();
}
