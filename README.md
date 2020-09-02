# CK3 Save

CK3 Save is a library to ergonomically work with Crusader Kings 3 (CK3) saves (ironman + regular).

```rust
use Ck3save::{Ck3Extractor};

let data = std::fs::read("assets/saves/eng.txt.compressed.Ck3")?;
let (save, encoding) = Ck3Extractor::extract_save(&data[..])?;
assert_eq!(encoding, Encoding::Text);
assert_eq!(save.gamestate.meta_data.version, String::from("1.0.2"));
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
