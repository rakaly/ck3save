/*!
# CK3 Save

CK3 Save is a library to ergonomically work with Crusader Kings 3 (CK3) saves (ironman + regular).

```rust
use ck3save::{
    models::{Gamestate, HeaderBorrowed},
    Ck3File, Encoding,
};

let data = std::fs::read("assets/saves/Jarl_Ivar_of_the_Isles_867_01_01.ck3")?;
let file = Ck3File::from_slice(&data)?;
assert_eq!(file.encoding(), Encoding::TextZip);

let resolver = std::collections::HashMap::<u16, &str>::new();
let mut zip_sink = Vec::new();
let parsed_file = file.parse(&mut zip_sink)?;
let save: Gamestate = parsed_file.deserializer(&resolver).deserialize()?;
assert_eq!(save.meta_data.version, String::from("1.0.2"));
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Ironman

Ironman saves are supported through a provided `TokenResolver`. Per PDS counsel, the data to construct such a `TokenResolver` is not distributed here.

*/

mod ck3date;
mod deflate;
mod errors;
mod extraction;
pub mod file;
pub(crate) mod flavor;
mod header;
mod melt;
pub mod models;

pub use ck3date::*;
pub use errors::*;
pub use extraction::*;
#[doc(inline)]
pub use file::Ck3File;
pub use header::*;
pub use jomini::binary::{BasicTokenResolver, FailedResolveStrategy};
pub use melt::*;
