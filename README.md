# memsizes

Type-safe memory size newtypes for Rust with checked conversions and arithmetic.

Stop mixing up bytes and mebibytes, or accidentally passing a decimal gigabyte where a binary one was expected.
`memsizes` gives each unit its own type so the compiler catches the mistakes for you.

## Units

| Binary (IEC) | Decimal (SI) | Base    |
|--------------|--------------|---------|
| `KiB` (1024) | `KB` (1000) | `Bytes` |
| `MiB` (1024²) | `MB` (1000²) | |
| `GiB` (1024³) | `GB` (1000³) | |
| `TiB` (1024⁴) | `TB` (1000⁴) | |
| `PiB` (1024⁵) | `PB` (1000⁵) | |
| `EiB` (1024⁶) | `EB` (1000⁶) | |

## Usage

```rust
use memsizes::{GiB, MiB, MB, MemorySize, Rounding};

let mem = GiB::from_units(2);

// Exact conversion (binary → binary)
let mib: MiB = mem.to_exact().unwrap();
assert_eq!(mib.count(), 2048);

// Rounded conversion (binary → decimal)
let mb = mem.to_rounded::<MB>(Rounding::Ceil).unwrap();

// Checked arithmetic (both operands must be the same type)
let total = mib.checked_add(MiB::from_units(512)).unwrap();
assert_eq!(total.count(), 2560);
```

All conversions go through `Bytes` internally and use checked arithmetic — overflows return errors instead of wrapping.

## Features

- Zero-cost newtypes over `u64`
- Compile-time unit safety between binary and decimal sizes
- Exact conversions (`to_exact`) and rounded conversions (`to_rounded`) with floor/ceil/nearest modes
- Checked and saturating add/sub
- Optional serde support via `features = ["serde"]`

## License

MIT
