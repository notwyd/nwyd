use crate::device::Gpu;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]

pub struct GeometryUploadStats {
    pub uploaded_bytes: u64,
    pub copied_bytes: u64,
    pub allocations: usize,
}

pub struct VertexCopy<'a> {
    pub source: &'a wgpu::Buffer,
    pub source_offset: u64,
    pub destination_offset: u64,
    pub size: u64,
}

pub fn stage_vertices<T: bytemuck::Pod>(gpu: &Gpu, vertices: &[T]) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;

    gpu.device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: crate::gpu_label("unpublished zone vertices"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::COPY_SRC,
        })
}

pub(crate) struct GeometryBuffer {
    pub buffer: wgpu::Buffer,
    label: String,
    usage: wgpu::BufferUsages,
    len: usize,
    stats: GeometryUploadStats,
}

impl GeometryBuffer {
    pub fn stats(&self) -> GeometryUploadStats {
        self.stats
    }

    pub fn patch(&mut self, gpu: &Gpu, offset: usize, bytes: &[u8]) {
        assert!(offset.is_multiple_of(4) && bytes.len().is_multiple_of(4));

        assert!(offset <= self.len && bytes.len() <= self.len - offset);

        if bytes.is_empty() {
            return;
        }

        gpu.queue.write_buffer(&self.buffer, offset as u64, bytes);

        self.stats.uploaded_bytes += bytes.len() as u64;
    }

    pub fn new(gpu: &Gpu, label: &str, capacity: u64, usage: wgpu::BufferUsages) -> Self {
        let usage = usage | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC;

        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: crate::gpu_label(label),
            size: capacity.max(4).next_multiple_of(4),
            usage,
            mapped_at_creation: false,
        });

        Self {
            buffer,
            label: crate::gpu_label(label).unwrap_or_default().into(),
            usage,
            len: 0,
            stats: GeometryUploadStats {
                allocations: 1,
                ..Default::default()
            },
        }
    }

    pub fn upload(&mut self, gpu: &Gpu, bytes: &[u8], append: bool) {
        self.upload_copies(gpu, bytes, append, &[]);
    }

    pub fn upload_copies(
        &mut self,
        gpu: &Gpu,
        bytes: &[u8],
        append: bool,
        copies: &[VertexCopy<'_>],
    ) {
        let append = append && bytes.len() >= self.len;

        if append && bytes.len() == self.len {
            return;
        }

        let size = bytes.len().next_multiple_of(4) as u64;

        if size > self.buffer.size() {
            let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: crate::gpu_label(&self.label),
                size: size.next_power_of_two(),
                usage: self.usage,
                mapped_at_creation: false,
            });

            if append && self.len != 0 {
                let mut encoder =
                    gpu.device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: crate::gpu_label("grow resident geometry"),
                        });

                let copied = self.len.next_multiple_of(4) as u64;

                encoder.copy_buffer_to_buffer(&self.buffer, 0, &buffer, 0, copied);

                gpu.queue.submit([encoder.finish()]);

                self.stats.copied_bytes += copied;
            }

            self.buffer = buffer;

            self.stats.allocations += 1;
        }

        let start = if append { self.len / 4 * 4 } else { 0 };

        let end = bytes.len() / 4 * 4;

        let mut cursor = start;

        let mut encoder = None;

        for copy in copies {
            let copy_end = copy
                .destination_offset
                .checked_add(copy.size)
                .expect("vertex copy range overflow");

            let from = copy.destination_offset.max(start as u64);

            let to = copy_end.min(end as u64);

            if from >= to {
                continue;
            }

            let (from, to) = (from as usize, to as usize);

            assert!(from >= cursor && from.is_multiple_of(4) && to.is_multiple_of(4));

            if from > cursor {
                gpu.queue
                    .write_buffer(&self.buffer, cursor as u64, &bytes[cursor..from]);

                self.stats.uploaded_bytes += (from - cursor) as u64;
            }

            let encoder = encoder
                .get_or_insert_with(|| gpu.device.create_command_encoder(&Default::default()));

            encoder.copy_buffer_to_buffer(
                copy.source,
                copy.source_offset + from as u64 - copy.destination_offset,
                &self.buffer,
                from as u64,
                (to - from) as u64,
            );

            self.stats.copied_bytes += (to - from) as u64;

            cursor = to;
        }

        if end > cursor {
            gpu.queue
                .write_buffer(&self.buffer, cursor as u64, &bytes[cursor..end]);

            self.stats.uploaded_bytes += (end - cursor) as u64;
        }

        if end < bytes.len() {
            let mut tail = [0; 4];

            tail[..bytes.len() - end].copy_from_slice(&bytes[end..]);

            gpu.queue.write_buffer(&self.buffer, end as u64, &tail);

            self.stats.uploaded_bytes += 4;
        }

        self.len = bytes.len();

        if let Some(encoder) = encoder {
            gpu.queue.submit([encoder.finish()]);
        }
    }
}

pub(crate) struct GeometryBuffers {
    pub vertices: GeometryBuffer,
    pub indices: GeometryBuffer,
}

impl GeometryBuffers {
    pub fn new(gpu: &Gpu, name: &str, vertices: u64, indices: u64) -> Self {
        Self {
            vertices: GeometryBuffer::new(
                gpu,
                &format!("{name} vertices"),
                vertices,
                wgpu::BufferUsages::VERTEX,
            ),
            indices: GeometryBuffer::new(
                gpu,
                &format!("{name} indices"),
                indices,
                wgpu::BufferUsages::INDEX,
            ),
        }
    }

    pub fn upload(&mut self, gpu: &Gpu, vertices: &[u8], indices: &[u16], append: bool) {
        self.upload_copies(gpu, vertices, indices, append, &[]);
    }

    pub fn upload_copies(
        &mut self,
        gpu: &Gpu,
        vertices: &[u8],
        indices: &[u16],
        append: bool,
        copies: &[VertexCopy<'_>],
    ) {
        let append = append
            && vertices.len() >= self.vertices.len
            && std::mem::size_of_val(indices) >= self.indices.len;

        self.vertices.upload_copies(gpu, vertices, append, copies);

        self.indices
            .upload(gpu, bytemuck::cast_slice(indices), append);
    }

    pub fn stats(&self) -> GeometryUploadStats {
        GeometryUploadStats {
            uploaded_bytes: self.vertices.stats.uploaded_bytes + self.indices.stats.uploaded_bytes,
            copied_bytes: self.vertices.stats.copied_bytes + self.indices.stats.copied_bytes,
            allocations: self.vertices.stats.allocations + self.indices.stats.allocations,
        }
    }
}
