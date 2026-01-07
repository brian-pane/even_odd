@group(0) @binding(0)
var<storage, read> input: u32;

@group(0) @binding(1)
var<storage, read_write> output: u32;

@compute @workgroup_size(64)
fn is_even(@builtin(global_invocation_id) global_id: vec3<u32>) {
    const BATCH_SIZE = 131072u;
    var known_even = global_id.x * BATCH_SIZE;
    var haystack = vec4u(known_even, known_even + 2, known_even + 4, known_even + 6);
    var needle = vec4u (input, input, input, input);
    const step = vec4u(8, 8, 8, 8);
    var count = BATCH_SIZE / 8;
    for (; count != 0; count -= 1) {
        if any(needle == haystack) {
            output = 1;
        }
        haystack += step;
    }
}
