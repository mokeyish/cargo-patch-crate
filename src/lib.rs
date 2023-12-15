//!
//! patch-crate lets rust app developer instantly make and keep fixes to crate dependencies.
//! It's a vital band-aid for those of us living on the bleeding edge.
//!
//! # Installation
//!
//! Simply run:
//! ```sh
//! cargo install patch-crate
//! ```
//!
//! # Usage
//!
//! To patch dependency one has to add the following
//! to `Cargo.toml`
//!
//! ```toml
//! [package.metadata.patch]
//! crates = ["serde"]
//! ```
//!
//! It specifies which dependency to patch (in this case
//! serde). Running:
//!
//! ```sh
//! cargo patch-crate
//! ```
//!
//! will download the sede package specified in the
//! dpendency section to the `target/patch` folder.
//!  
//! Then override the dependency using
//! `replace` like this
//!
//! ```toml
//! [patch.crates-io]
//! serde = { path = './target/patch/serde-1.0.110' }
//! ```
//!
//! fix a bug in './target/patch/serde-1.0.110' directly.
//!
//! run following to create a `patches/serde+1.0.110.patch` file
//! ```sh
//! cargo patch-crate serde
//! ```
//!
//! commit the patch file to share the fix with your team
//! ```sh
//! git add patches/serde+1.0.110.patch
//! git commit -m "fix broken-serde in serde"
//! ```

use anyhow::{anyhow, Ok, Result};
use cargo::{
    core::{
        package::{Package, PackageSet},
        registry::PackageRegistry,
        resolver::{features::CliFeatures, HasDevUnits},
        Resolve, Workspace,
    },
    ops::{get_resolved_packages, load_pkg_lockfile, resolve_with_previous},
    util::{config::Config, important_paths::find_root_manifest_for_wd},
};
use clap::Parser;
use fs_extra::dir::{copy, CopyOptions};
use log::*;
use std::{
    collections::HashSet,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

const PATCH_EXT: &str = "patch";

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    crates: Vec<String>,
    #[arg(short, long)]
    force: bool,
}

trait PackageExt {
    fn slug(&self) -> Result<&str>;
    fn patch_target_path(&self, workspace: &Workspace<'_>) -> Result<PathBuf>;
}

impl PackageExt for Package {
    fn slug(&self) -> Result<&str> {
        if let Some(name) = self.root().file_name().and_then(|s| s.to_str()) {
            Ok(name)
        } else {
            Err(anyhow!("Dependency Folder does not have a name"))
        }
    }

    fn patch_target_path(&self, workspace: &Workspace<'_>) -> Result<PathBuf> {
        let slug = self.slug()?;
        let patch_target_path = workspace.patch_target_folder().join(slug);
        Ok(patch_target_path)
    }
}

trait WorkspaceExt {
    fn patches_folder(&self) -> PathBuf;
    fn patch_target_folder(&self) -> PathBuf;
    fn patch_target_tmp_folder(&self) -> PathBuf;
    fn clean_patch_folder(&self) -> Result<()>;
}

impl WorkspaceExt for Workspace<'_> {
    fn patches_folder(&self) -> PathBuf {
        self.root().join("patches/")
    }
    fn patch_target_folder(&self) -> PathBuf {
        self.root().join("target/patch/")
    }
    fn patch_target_tmp_folder(&self) -> PathBuf {
        self.root().join("target/patch-tmp/")
    }

    fn clean_patch_folder(&self) -> Result<()> {
        fs::remove_dir_all(self.patch_target_folder())?;
        Ok(())
    }
}

fn resolve_ws<'a>(ws: &Workspace<'a>) -> Result<(PackageSet<'a>, Resolve)> {
    let mut registry = PackageRegistry::new(ws.config())?;
    registry.lock_patches();
    let resolve = {
        let prev = load_pkg_lockfile(ws)?;
        let resolve: Resolve = resolve_with_previous(
            &mut registry,
            ws,
            &CliFeatures::new_all(true),
            HasDevUnits::No,
            prev.as_ref(),
            None,
            &[],
            false,
        )?;
        resolve
    };
    let packages = get_resolved_packages(&resolve, registry)?;
    Ok((packages, resolve))
}

fn copy_package(pkg: &Package, patch_target_folder: &Path, overwrite: bool) -> Result<PathBuf> {
    fs::create_dir_all(patch_target_folder)?;
    let options = CopyOptions::new();
    let patch_target_path = patch_target_folder.join(pkg.slug()?);
    if patch_target_path.exists() {
        if overwrite {
            info!("crate: {}, copy to {:?}", pkg.name(), &patch_target_folder);
            fs::remove_dir_all(&patch_target_path)?;
        } else {
            info!(
                "crate: {}, skip, {:?} already exists.",
                pkg.name(),
                &patch_target_path
            );
            return Ok(patch_target_path);
        }
    }
    let _ = copy(pkg.root(), patch_target_folder, &options)?;
    Ok(patch_target_path)
}

fn find_cargo_toml(path: &Path) -> Result<PathBuf> {
    let path = fs::canonicalize(path)?;
    find_root_manifest_for_wd(&path)
}

pub fn run() -> anyhow::Result<()> {
    let args = {
        let mut args = Cli::parse();
        if let Some(idx) = args.crates.iter().position(|c| c == "patch-crate") {
            args.crates.remove(idx);
        }
        args
    };

    let config = Config::default()?;
    let _lock = config.acquire_package_cache_lock()?;

    let cargo_toml_path = find_cargo_toml(&PathBuf::from("."))?;

    let workspace = Workspace::new(&cargo_toml_path, &config)?;

    let patches_folder = workspace.patches_folder();

    let patch_target_folder = workspace.patch_target_folder();
    let patch_target_tmp_folder = workspace.patch_target_tmp_folder();

    let (pkg_set, resolve) = resolve_ws(&workspace)?;

    if !args.crates.is_empty() {
        info!("starting patch creation.");
        if !patches_folder.exists() {
            fs::create_dir_all(&patches_folder)?;
        }
        for n in args.crates.iter() {
            // make patch
            info!("crate: {}, starting patch creation.", n);
            let pkg_id = resolve.query(n)?;
            let pkg = pkg_set.get_one(pkg_id)?;
            let patch_target_path = pkg.patch_target_path(&workspace)?;
            let patch_target_tmp_path = copy_package(pkg, &patch_target_tmp_folder, true)?;
            git::init(&patch_target_tmp_path)?;
            git::destroy(&patch_target_path)?;
            copy(
                &patch_target_path,
                &patch_target_tmp_folder,
                &CopyOptions::new().overwrite(true).copy_inside(true),
            )?;
            let patch_file = patches_folder.join(format!(
                "{}+{}.{}",
                pkg_id.name(),
                pkg_id.version(),
                PATCH_EXT
            ));
            git::create_patch(&patch_target_tmp_path, &patch_file)?;
            fs::remove_dir_all(&patch_target_tmp_folder)?;
            info!("crate: {}, create patch successfully, {:?}", n, &patch_file);
        }
    } else {
        // apply patch
        info!("applying patch");

        let custom_metadata = workspace.custom_metadata().into_iter().chain(
            workspace
                .members()
                .flat_map(|member| member.manifest().custom_metadata()),
        );

        let mut crates_to_patch = custom_metadata
            .flat_map(|m| {
                m.as_table()
                    .and_then(|table| table.get("patch"))
                    .into_iter()
                    .flat_map(|patch| patch.as_table())
                    .flat_map(|patch| patch.get("crates"))
                    .filter_map(|crates| crates.as_array())
            })
            .flatten()
            .flat_map(|s| s.as_str())
            .map(|n| resolve.query(n).and_then(|id| pkg_set.get_one(id)))
            .collect::<Result<HashSet<_>>>()?;

        if args.force {
            info!("Cleaning up patch folder.");
            workspace.clean_patch_folder()?;
        }

        if patches_folder.exists() {
            for entry in fs::read_dir(patches_folder)? {
                let entry = entry?;
                if entry.metadata()?.is_file()
                    && entry.path().extension() == Some(OsStr::new(PATCH_EXT))
                {
                    let patch_file = entry.path();
                    let filename = patch_file
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .ok_or(anyhow!("Patch file does not have a name"))?;

                    if let Some((pkg_name, _version)) = filename.split_once('+') {
                        let pkg_id = resolve.query(pkg_name)?;
                        let pkg = pkg_set.get_one(pkg_id)?;
                        if !crates_to_patch.contains(&pkg) {
                            warn!(
                                "crate: {}, {} is not in the [package.metadata.patch] section of Cargo.toml. Did you forget to add it?",
                                pkg_name, pkg_name
                            );
                            continue;
                        }

                        let patch_target_path = pkg.patch_target_path(&workspace)?;
                        if !patch_target_path.exists() {
                            copy_package(pkg, &patch_target_folder, args.force)?;
                            info!("crate: {}, applying patch started.", pkg_name);
                            git::init(&patch_target_path)?;
                            git::apply(&patch_target_path, &patch_file)?;
                            git::destroy(&patch_target_path)?;
                            info!(
                                "crate: {}, successfully applied patch {:?}.",
                                pkg_name, patch_file
                            );
                        } else {
                            info!("crate: {}, skip applying patch, {:?} already exists. Did you forget to add `--force`?", pkg_name, patch_target_path);
                        }
                        crates_to_patch.remove(pkg);
                    }
                }
            }
        }
        for pkg in crates_to_patch {
            copy_package(pkg, &patch_target_folder, args.force)?;
        }
    }

    info!("Done");
    Ok(())
}

mod log {
    pub use paris::*;
}

mod git {
    use std::{ffi::OsStr, fs, path::Path, process::Command};

    pub fn init(repo_dir: &Path) -> anyhow::Result<()> {
        Command::new("git")
            .current_dir(repo_dir)
            .args(["init"])
            .output()?;
        Command::new("git")
            .current_dir(repo_dir)
            .args(["add", "."])
            .output()?;
        Command::new("git")
            .current_dir(repo_dir)
            .args(["commit", "-m", "zero"])
            .output()?;
        Ok(())
    }

    pub fn apply(repo_dir: &Path, patch_file: &Path) -> anyhow::Result<()> {
        Command::new("git")
            .current_dir(repo_dir)
            .args([OsStr::new("apply"), OsStr::new(patch_file)])
            .output()?;
        Ok(())
    }
    pub fn destroy(repo_dir: &Path) -> anyhow::Result<()> {
        let git_dir = repo_dir.join(".git");
        if git_dir.exists() {
            fs::remove_dir_all(git_dir)?;
        }
        Ok(())
    }
    pub fn create_patch(repo_dir: &Path, patch_file: &Path) -> anyhow::Result<()> {
        Command::new("git")
            .current_dir(repo_dir)
            .args(["add", "."])
            .output()?;

        let out = Command::new("git")
            .current_dir(repo_dir)
            .args([OsStr::new("diff"), OsStr::new("--staged")])
            .output()?;

        if out.status.success() {
            fs::write(patch_file, out.stdout)?;
        }
        Ok(())
    }
}
