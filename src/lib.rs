use rayon::prelude::*;
use std::num::NonZeroU64;
use std::sync::{Arc, Mutex};
use wgpu::util::DeviceExt;

pub fn is_even(num: u32) -> bool {
    let mut even = false;
    let mut known_even = Some(0u32);
    while let Some(known) = known_even {
        if num == known {
            even = true;
        }
        known_even = known.checked_add(2);
    }
    even
}

const LOG2_BLOCK_SIZE: usize = 12;

const BLOCK_SIZE: usize = 1 << LOG2_BLOCK_SIZE;

const TOTAL_BLOCKS: usize = 1 << (32 - LOG2_BLOCK_SIZE);

fn partition_chunks(num_chunks: usize) -> Vec<(usize, usize)> {
    let num_chunks = core::cmp::min(num_chunks, TOTAL_BLOCKS);
    let mut chunks = Vec::with_capacity(num_chunks);
    let mut start_block: usize = 0;
    let blocks_per_chunk = TOTAL_BLOCKS / num_chunks;
    for i in 0..num_chunks {
        let num_blocks = if i + 1 == num_chunks {
            TOTAL_BLOCKS - start_block
        } else {
            blocks_per_chunk
        };
        chunks.push((start_block, num_blocks));
        start_block += num_blocks;
    }
    chunks
}

pub fn is_even_rayon(num: u32) -> bool {
    // Split the range [0..u32::MAX] into N chunks, where N = num CPU cores * CHUNK_MULTIPLIER.
    // A CHUNK_MULTIPLIER of 1 is optimal if all CPU cores on the system are equally fast, but
    // if some cores are faster than others (e.g. on a processor with a mix of performance cores
    // and efficiency cores) we often can get better results with a larger number of smaller chunks.
    const CHUNK_MULTIPLIER: usize = 10;
    let chunks =
        partition_chunks(std::thread::available_parallelism().unwrap().get() * CHUNK_MULTIPLIER);
    let even = Arc::new(Mutex::new(false));
    let worker_even = Arc::clone(&even);
    chunks
        .par_iter()
        .for_each(move |&(start_block, num_blocks)| {
            let mut local_even = false;
            let mut known_even = (start_block * BLOCK_SIZE) as u32;
            for _ in 0..BLOCK_SIZE * num_blocks / 2 {
                if known_even == num {
                    local_even = true;
                }
                known_even = known_even.wrapping_add(2);
            }
            if local_even {
                *worker_even.lock().unwrap() = true;
            }
        });

    *even.lock().unwrap()
}

pub struct WgpuEvenOdd {
    queue: wgpu::Queue,
    pub shader: wgpu::ShaderModule,
    pub device: wgpu::Device,
}

impl WgpuEvenOdd {
    pub fn new() -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = futures::executor::block_on(
            instance.request_adapter(&wgpu::RequestAdapterOptions::default()),
        )
        .unwrap();
        if !adapter
            .get_downlevel_capabilities()
            .flags
            .contains(wgpu::DownlevelFlags::COMPUTE_SHADERS)
        {
            panic!("WGPU adapter does not support compute shaders");
        }
        let device_descriptor = wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            trace: wgpu::Trace::Off,
        };
        let (device, queue) =
            futures::executor::block_on(adapter.request_device(&device_descriptor)).unwrap();
        let shader = device.create_shader_module(wgpu::include_wgsl!("even_odd.wgsl"));
        Self {
            device,
            queue,
            shader,
        }
    }

    pub fn is_even(&self, num: u32) -> bool {
        let input_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&[num]),
                usage: wgpu::BufferUsages::STORAGE,
            });
        let output_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&[0u32]),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            });
        let output_for_cpu = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: input_buffer.size(),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let bind_group_layout =
            self.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                min_binding_size: Some(NonZeroU64::new(4).unwrap()),
                                has_dynamic_offset: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: false },
                                min_binding_size: Some(NonZeroU64::new(4).unwrap()),
                                has_dynamic_offset: false,
                            },
                            count: None,
                        },
                    ],
                });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output_buffer.as_entire_binding(),
                },
            ],
        });
        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&bind_group_layout],
                immediate_size: 0,
            });
        let pipeline = self
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                module: &self.shader,
                entry_point: Some("is_even"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            const WORKGROUP_SIZE: u64 = 131072; // must match even_odd.wgsl
            const NUM_WORKGROUPS: u32 = ((u32::MAX as u64 + 1) / WORKGROUP_SIZE) as _;
            compute_pass.dispatch_workgroups(NUM_WORKGROUPS, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&output_buffer, 0, &output_for_cpu, 0, output_buffer.size());
        let command_buffer = encoder.finish();
        self.queue.submit([command_buffer]);
        let output_slice = output_for_cpu.slice(..);
        output_slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        let data = output_slice.get_mapped_range();
        let result: &[u32] = bytemuck::cast_slice(&data);
        result[0] != 0
    }
}

impl Default for WgpuEvenOdd {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::hint::black_box;

    #[test]
    fn single_threaded() {
        assert_eq!(is_even(black_box(0)), true);
        assert_eq!(is_even(black_box(1)), false);
        assert_eq!(is_even(black_box(2)), true);
        assert_eq!(is_even(black_box(9_999_999)), false);
        assert_eq!(is_even(black_box(100_000_000)), true);
        assert_eq!(is_even(black_box(3_000_000_004)), true);
        assert_eq!(is_even(u32::MAX - 1), true);
        assert_eq!(is_even(u32::MAX), false);
    }

    #[test]
    fn rayon() {
        assert_eq!(is_even_rayon(black_box(0)), true);
        assert_eq!(is_even_rayon(black_box(1)), false);
        assert_eq!(is_even_rayon(black_box(2)), true);
        assert_eq!(is_even_rayon(black_box(9_999_999)), false);
        assert_eq!(is_even_rayon(black_box(100_000_000)), true);
        assert_eq!(is_even_rayon(black_box(3_000_000_004)), true);
        assert_eq!(is_even_rayon(u32::MAX - 1), true);
        assert_eq!(is_even_rayon(u32::MAX), false);
    }

    #[test]
    fn wgpu() {
        let even_odd = WgpuEvenOdd::new();
        assert_eq!(even_odd.is_even(black_box(0)), true);
        assert_eq!(even_odd.is_even(black_box(1)), false);
        assert_eq!(even_odd.is_even(black_box(2)), true);
        assert_eq!(even_odd.is_even(black_box(9_999_999)), false);
        assert_eq!(even_odd.is_even(black_box(100_000_000)), true);
        assert_eq!(even_odd.is_even(black_box(3_000_000_004)), true);
        assert_eq!(even_odd.is_even(black_box(4_294_767_290)), true);
        assert_eq!(even_odd.is_even(u32::MAX - 1), true);
        assert_eq!(even_odd.is_even(u32::MAX), false);
    }
}
