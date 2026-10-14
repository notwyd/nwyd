use crate::device::Gpu;

pub fn strided<T: bytemuck::Pod>(
    scratch: &mut Vec<u8>,
    stride: u32,
    items: impl ExactSizeIterator<Item = T>,
) -> &[u8] {
    let (stride, block) = (stride as usize, size_of::<T>());

    assert!(
        stride >= block,
        "a {stride}-byte slot cannot hold a {block}-byte block"
    );

    scratch.resize(items.len() * stride, 0);

    for (slot, item) in scratch.chunks_exact_mut(stride).zip(items) {
        slot[..block].copy_from_slice(bytemuck::bytes_of(&item));
    }

    scratch
}

pub fn upload<T: bytemuck::Pod>(
    gpu: &Gpu,
    buffer: &wgpu::Buffer,
    scratch: &mut Vec<u8>,
    stride: u32,
    items: &[T],
) {
    if items.is_empty() {
        return;
    }

    if stride as usize == size_of::<T>() {
        gpu.queue
            .write_buffer(buffer, 0, bytemuck::cast_slice(items));

        return;
    }

    gpu.queue
        .write_buffer(buffer, 0, strided(scratch, stride, items.iter().copied()));
}

pub fn indices<'a>(scratch: &'a mut Vec<u16>, indices: &[u16]) -> &'a [u16] {
    scratch.clear();

    scratch.extend_from_slice(indices);

    if !scratch.len().is_multiple_of(2) {
        scratch.push(0);
    }

    scratch
}
