/*!
# CK3 Save

CK3 Save is a library to ergonomically work with Crusader Kings 3 (CK3) saves (ironman + regular).

```rust
use ck3save::{models::Gamestate, Ck3File, Encoding};

let file = std::fs::File::open("assets/saves/Jarl_Ivar_of_the_Isles_867_01_01.ck3")?;
let mut file = Ck3File::from_file(file)?;
assert_eq!(file.encoding(), Encoding::TextZip);

let resolver = std::collections::HashMap::<u16, &str>::new();
let save = file.parse_save(&resolver)?;
assert_eq!(save.meta_data.version, String::from("1.0.2"));
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Ironman

Ironman saves are supported through a provided `TokenResolver`. Per PDS counsel, the data to construct such a `TokenResolver` is not distributed here.

*/

mod ck3date;
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
