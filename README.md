# memsizes

Type-safe memory size newtypes for Rust with checked conversions and arithmetic.

Stop mixing up bytes and mebibytes, or accidentally passing a decimal gigabyte where a binary one was expected.
`memsizes` gives each unit its own type so the compiler catches the mistakes for you.

## Units

| Binary (IEC) | Decimal (SI) | Base    |
|--------------|--------------|---------|
| `KiB` (1024) | `KB` (1000) | `Bytes` |
| `MiB` (1024^2) | `MB` (1000^2) | |
| `GiB` (1024^3) | `GB` (1000^3) | |

## Usage

```rust
use memsizes::{GiB, MiB, MB, MemorySize, Rounding};

let mem = GiB::from_units(2);

// Exact conversion (binary → binary)
let mib: MiB = mem.to_exact::<MiB>().unwrap();
assert_eq!(mib.units(), 2048);

// Rounded conversion (binary → decimal)
let mb = mem.to_rounded::<MB>(Rounding::Ceil).unwrap();

// Checked arithmetic
let total = mib.checked_add(MiB::from_units(512)).unwrap();
assert_eq!(total.units(), 2560);
```

All conversions go through `Bytes` internally and use checked arithmetic — overflows return errors instead of wrapping.

## Features

- Zero-cost newtypes over `u64`
- Compile-time unit safety between binary and decimal sizes
- Exact conversions (`to_exact`) and rounded conversions (`to_rounded`) with floor/ceil/nearest modes
- Checked and saturating add/sub
- Serde support out of the box

## License

MIT
