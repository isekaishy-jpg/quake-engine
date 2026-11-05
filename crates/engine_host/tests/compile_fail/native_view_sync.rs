use engine_host::NativeWindowRef;

fn require_sync<T: Sync>() {}

fn main() {
    require_sync::<NativeWindowRef<'static>>();
}
