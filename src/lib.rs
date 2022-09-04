/*!
# CK3 Save

CK3 Save is a library to ergonomically work with Crusader Kings 3 (CK3) saves (ironman + regular).

```rust
use ck3save::{
    models::{Gamestate, HeaderBorrowed},
    Ck3File, Encoding, EnvTokens,
};

let data = std::fs::read("assets/saves/Jarl_Ivar_of_the_Isles_867_01_01.ck3")?;
let file = Ck3File::from_slice(&data)?;
assert_eq!(file.encoding(), Encoding::TextZip);

let meta = file.parse_metadata()?;
let header: HeaderBorrowed = meta.deserializer().build(&EnvTokens)?;

let mut zip_sink = Vec::new();
let parsed_file = file.parse(&mut zip_sink)?;
let save: Gamestate = parsed_file.deserializer().build(&EnvTokens)?;
assert_eq!(save.meta_data.version, String::from("1.0.2"));
assert_eq!(header.meta_data.version, String::from("1.0.2"));
# Ok::<(), Box<dyn std::error::Error>>(())
```

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
mod deflate;
mod errors;
mod extraction;
pub mod file;
pub(crate) mod flavor;
mod header;
mod melt;
pub mod models;
mod tokens;

pub use ck3date::*;
pub use errors::*;
pub use extraction::*;
#[doc(inline)]
pub use file::Ck3File;
pub use header::*;
pub use jomini::binary::FailedResolveStrategy;
pub use melt::*;
pub use tokens::EnvTokens;
