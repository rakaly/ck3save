/*!
# CK3 Save

CK3 Save is a library to ergonomically work with Crusader Kings 3 (CK3) saves (ironman + regular).

```rust
use ck3save::{Ck3Extractor, Encoding};
use std::io::Cursor;

let data = std::fs::read("assets/saves/Jarl_Ivar_of_the_Isles_867_01_01.ck3")?;
let reader = Cursor::new(&data[..]);
let (save, encoding) = Ck3Extractor::extract_save(reader)?;
assert_eq!(encoding, Encoding::TextZip);
assert_eq!(save.meta_data.version, String::from("1.0.2"));
# Ok::<(), Box<dyn std::error::Error>>(())
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
*/

mod ck3date;
mod errors;
mod extraction;
mod melt;
pub mod models;
mod tokens;

pub use ck3date::*;
pub use errors::*;
pub use extraction::*;
pub use jomini::FailedResolveStrategy;
pub use melt::*;
