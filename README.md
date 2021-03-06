![ci](https://github.com/rakaly/ck3save/workflows/ci/badge.svg) [![](https://docs.rs/ck3save/badge.svg)](https://docs.rs/ck3save) [![Version](https://img.shields.io/crates/v/ck3save.svg?style=flat-square)](https://crates.io/crates/ck3save)

# CK3 Save

CK3 Save is a library to ergonomically work with Crusader Kings III (CK3) saves (ironman + regular).

```rust
use ck3save::{Ck3Extractor, Encoding};
use std::io::Cursor;

let data = std::fs::read("assets/saves/Jarl_Ivar_of_the_Isles_867_01_01.ck3")?;
let reader = Cursor::new(&data[..]);
let (save, encoding) = Ck3Extractor::extract_save(reader)?;
assert_eq!(encoding, Encoding::TextZip);
assert_eq!(save.meta_data.version, String::from("1.0.2"));
```

`Ck3Extractor` will deserialize both plaintext (used for mods, multiplayer,
non-ironman saves) and binary (ironman) encoded saves into the same structure.

## Ironman

By default, ironman saves will not be decoded properly.

To enable support, one must supply an environment variable
(`CK3_IRONMAN_TOKENS`) that points to a newline delimited
text file of token descriptions. For instance:

```ignore
0xffff my_test_token
0xeeee my_test_token2
```

In order to comply with legal restrictions, I cannot share the list of
tokens. I am also restricted from divulging how the list of tokens can be derived.
