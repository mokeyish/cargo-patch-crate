## Patch-Crate

[![](https://img.shields.io/crates/v/patch-crate.svg)](https://crates.io/crates/patch-crate)

patch-crate lets rust app developer instantly make and keep fixes to rust crate dependencies.
It's a vital band-aid for those of us living on the bleeding edge.

```sh
# fix a bug in one of your dependencies
vim target/patch/brokencrate

# run patch-crate to create a .patch file
cargo patch-crate some-crate

# commit the patch file to share the fix with your team
git add patches/some-crate+3.14.15.patch
git commit -m "fix broken_file.rs in some-crate"
```

Checkout our example at [here](https://github.com/mokeyish/cargo-patch-crate-example).

## Get started

1. Install command `patch-crate`

   ```sh
   cargo install patch-crate
   ```

2. Add broken crate in your Cargo.toml

   ```toml

   [package.metadata.patch]
   crates = ["some-crate"]

   [patch.crates-io]
   some-crate = { path="./target/patch/some-crate-1.0.110" }
   ```

3. Download the crate's source code into `target/patch`

   ```sh
   cargo patch-crate
   ```

4. Fix the broken code in `target/patch/some-crate` directly.

5. Create a crate-patch

   ```sh
   cargo patch-crate some-crate
   ```

6. Commit the patch file to share the fix with your team

   ```sh
   git add patches/some-crate+1.0.110.patch
   git commit -m "fix broken-code in some-crate"
   ```

7. Instead of running cargo patch-crate its also possible to add a build.rs file like this:

   ```rust
   fn main() {
      println!("cargo:rerun-if-changed=Cargo.toml");
      patch_crate::run().expect("Failed while patching");
   }
   ```

   To make it work, add the patch-crate library to the build-dependencies
   
   ```toml
   patch-crate = "0.1"
   ```

## Command explanation

- `cargo patch-crate`
   
   Apply patch files in `./patches` to `./target/patch/crate-xxx` if it not exist.

- `cargo patch-crate --force`

   Clean up `./target/patch/` and apply patch files in `./patches` to `./target/patch/crate-xxx`.

- `crate patch-crate <crate name1> <crate name2> ...`

   Create patch file of specific crate from `./target/patch/crate-xxx` and save to `./patches`


## Credits

- [itmettkeDE/cargo-patch](https://github.com/itmettkeDE/cargo-patch)
- [ds300/patch-package](https://github.com/ds300/patch-package)

## License

Licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.