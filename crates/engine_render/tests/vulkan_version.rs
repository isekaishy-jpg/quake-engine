//! What a device needs to be picked.

use engine_render::{NEEDED_VULKAN, vulkan_is_new_enough};

#[test]
fn a_device_needs_vulkan_one_point_three() {
    assert_eq!(NEEDED_VULKAN, (1, 3));
    assert!(vulkan_is_new_enough((1, 3)));
    assert!(vulkan_is_new_enough((1, 4)));
    assert!(!vulkan_is_new_enough((1, 2)));
    assert!(!vulkan_is_new_enough((1, 0)));
}

/// By major version first, or a future 2.0 would read as older than 1.3.
#[test]
fn a_new_major_version_is_new_enough_whatever_its_minor() {
    assert!(vulkan_is_new_enough((2, 0)));
}
