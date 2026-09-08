# Performance baseline

Date: 2026-09-08  
Profile: optimized `bench`  
Input: 2,048 × 2,048 opaque grayscale gradient, scale 1, black-and-white palette

| Algorithm | Time | Throughput |
| --- | ---: | ---: |
| Floyd–Steinberg | 500.69 ms | 8.38 MP/s |
| Atkinson | 499.56 ms | 8.40 MP/s |
| Bayer 8×8, 8 colors | 1.33 s | 3.16 MP/s |

The first Bayer measurement took 6.56 s (0.64 MP/s). Precomputing the 28 linear palette segments
once per render reduced it to 1.33 s (3.16 MP/s), about 4.9 times faster. The pair search remains
the main Bayer cost, but this baseline does not justify a more complex method before tests on target
Windows computers.

The current full-frame implementation owns about 39 bytes per input pixel while it renders at
scale 1: source RGBA (4), reduced samples (16), linear work colors (12), selected colors (3), and
output RGBA (4). The 2,048 × 2,048 benchmark therefore needs about 156 MB (149 MiB), plus small
vector and allocator costs. A 40-million-pixel image could need about 1.56 GB. This is a risk for
the current image limit and must be reduced or enforced by a render-memory check before image I/O
is added.

Bayer does not need the extra linear error buffer. At scale 1, its main full-frame buffers use about
27 bytes per pixel, or about 108 MB (103 MiB) for the benchmark image.

Run the same baseline on the target Windows computers with:

```powershell
cargo bench -p dither-engine --bench error_diffusion
```

The times above are a development baseline, not a release target. Record the processor and memory
when collecting target-machine results.
