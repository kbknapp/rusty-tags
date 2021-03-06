#![allow(unused_assignments)]
#![feature(core, path_ext, exit_status, std_misc, str_char, convert)]

extern crate toml;
extern crate glob;
extern crate term;

use std::fs::{self, PathExt};
use std::env;
use std::path::{PathBuf, Path, AsPath};

use app_result::{AppResult, AppErr, app_err};
use dependencies::read_dependencies;
use types::{TagsRoot, TagsKind};

use tags::{
   update_tags,
   update_tags_and_check_for_reexports,
   create_tags,
   merge_tags
};

use dirs::rusty_tags_dir;

mod app_result;
mod dependencies;
mod dirs;
mod tags;
mod types;

fn main() 
{
   let mut args = env::args();
   let tags_kind =
      if args.len() == 2 {
         args.nth(1).and_then(|arg| {
            match arg.as_ref() {
               "vi"    => Some(TagsKind::Vi),
               "emacs" => Some(TagsKind::Emacs),
               _       => None
            }
         })
      }
      else {
         None
      };

   if let Some(tkind) = tags_kind {
      update_all_tags(&tkind).unwrap_or_else(|err| {
         write_to_stderr(&err);
         env::set_exit_status(1);
      });
   }
   else {
      println!("Usage:
   rusty-tags vi
   rusty-tags emacs");
   }
}

fn update_all_tags(tags_kind: &TagsKind) -> AppResult<()>
{
   let cwd = try!(env::current_dir());
   let cargo_dir = try!(find_cargo_toml_dir(&cwd));
   let tags_roots = try!(read_dependencies(&cargo_dir));

   let rust_std_lib_tags_file = try!(rust_std_lib_tags_file(tags_kind));

   for tags_root in tags_roots.iter() {
      let mut tag_files: Vec<PathBuf> = Vec::new();
      let mut tag_dir: Option<PathBuf> = None;

      match *tags_root {
         TagsRoot::Src { ref src_dir, ref dependencies } => {
            let mut src_tags = src_dir.clone();
            src_tags.push(tags_kind.tags_file_name());
            try!(create_tags(tags_kind, src_dir, &src_tags));
            tag_files.push(src_tags);

            for dep in dependencies.iter() {
               match update_tags(tags_kind, dep) {
                  Ok(tags) => tag_files.push(tags.tags_file),
                  Err(err) => write_to_stderr(&err)
               }
            }

            tag_dir = Some(src_dir.clone());
         },

         TagsRoot::Lib { ref src_kind, ref dependencies } => {
            let lib_tags = match update_tags_and_check_for_reexports(tags_kind, src_kind, dependencies) {
               Ok(tags) => {
                 if tags.is_up_to_date(tags_kind) {
                    continue;
                 }
                 else {
                    tags
                 }
               }

               Err(err) => {
                   write_to_stderr(&err);
                   continue;
               }
            };

            tag_files.push(lib_tags.tags_file);

            for dep in dependencies.iter() {
               match update_tags(tags_kind, dep) {
                  Ok(tags) => tag_files.push(tags.tags_file),
                  Err(err) => write_to_stderr(&err)
               }
            }

            tag_dir = Some(lib_tags.src_dir.clone());
         }
      }

      if tag_files.is_empty() || tag_dir.is_none() {
         continue;
      }

      if rust_std_lib_tags_file.as_path().is_file() {
         tag_files.push(rust_std_lib_tags_file.clone());
      }

      let mut tags_file = tag_dir.unwrap();
      tags_file.push(tags_kind.tags_file_name());

      try!(merge_tags(tags_kind, &tag_files, &tags_file));
   }

   Ok(())
}

/// Searches for a directory containing a `Cargo.toml` file starting at
/// `start_dir` and continuing the search upwards the directory tree
/// until a directory is found.
fn find_cargo_toml_dir(start_dir: &Path) -> AppResult<PathBuf>
{
   let mut dir = start_dir.to_path_buf();
   loop {
      if let Ok(files) = fs::read_dir(&dir) {
         for file in files {
            if let Ok(file) = file {
               if file.path().is_file() {
                  if let Some("Cargo.toml") = file.path().file_name().and_then(|s| s.to_str()) {
                     return Ok(dir);
                  }
               }
            }
         }
      }

      if ! dir.pop() {
         return Err(app_err(format!("Couldn't find 'Cargo.toml' starting at directory '{}'!", start_dir.display())));
      }
   }
}

/// the tags file containing the tags for the rust standard library
fn rust_std_lib_tags_file(tags_kind: &TagsKind) -> AppResult<PathBuf>
{
   let mut tags_file = try!(rusty_tags_dir());
   tags_file.push(&format!("rust-std-lib.{}", tags_kind.tags_file_extension()));

   Ok(tags_file)
}

fn write_to_stderr(err: &AppErr)
{
   if let Some(mut stderr) = term::stderr() {
      let _ = writeln!(&mut stderr, "rusty-tags: {}", err);
   }
}
