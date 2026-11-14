pub mod bin;
pub mod math;
pub mod png;
pub mod rng;

/// Reinterprets a slice of plain-old-data values as bytes (for GPU uploads and file IO).
pub fn as_bytes<T: Copy>(data: &[T]) -> &[u8] {
    // SAFETY: T is Copy plain data supplied by callers that use #[repr(C)] vertex types or integers.
    unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, std::mem::size_of_val(data)) }
}
