![ci](https://github.com/rakaly/ck3save/workflows/ci/badge.svg) [![](https://docs.rs/ck3save/badge.svg)](https://docs.rs/ck3save) [![Version](https://img.shields.io/crates/v/ck3save.svg?style=flat-square)](https://crates.io/crates/ck3save)

# CK3 Save

CK3 Save is a library to ergonomically work with Crusader Kings III (CK3) saves (ironman + regular).

```rust
use ck3save::{models::Gamestate, Ck3File, Encoding};

let file = std::fs::File::open("assets/saves/Jarl_Ivar_of_the_Isles_867_01_01.ck3")?;
let mut file = Ck3File::from_file(file)?;
assert_eq!(file.encoding(), Encoding::TextZip);

let resolver = std::collections::HashMap::<u16, &str>::new();
let save = file.parse_save(&resolver)?;
assert_eq!(save.meta_data.version, String::from("1.0.2"));
```

## Ironman

Ironman saves are supported through a provided `TokenResolver`. Per PDS counsel, the data to construct such a `TokenResolver` is not distributed here.
